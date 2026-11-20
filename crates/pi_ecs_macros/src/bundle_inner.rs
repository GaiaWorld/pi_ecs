//! 类型化派生展开和输入诊断。
use proc_macro::TokenStream;
#[path = "derive_path_inner.rs"]
mod derive_path;

#[cfg(test)]
#[test]
fn nested_fields_keep_their_positions_and_reject_misplaced_attributes() {
    let tokens = expand(syn::parse_quote! {
        #[bundle(crate = renamed)]
        struct Mixed<'a>(Number, #[bundle(nested)] Input<'a>, Flag);
    })
    .unwrap()
    .to_string();
    let first = tokens.find("write (self . 0)").unwrap();
    let nested = tokens.find("write (self . 1 ,").unwrap();
    let last = tokens.find("write (self . 2)").unwrap();
    assert!(first < nested && nested < last);
    assert!(expand(syn::parse_quote!(
        #[bundle(crate = renamed)]
        struct Bad(#[bundle(skip)] Number);
    ))
    .is_err());
}
pub(super) fn derive_bundle_inner(input: TokenStream) -> TokenStream {
    syn::parse(input)
        .and_then(expand)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(input: syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    use quote::{quote, quote_spanned};
    use syn::spanned::Spanned;
    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "Bundle requires a struct",
        ));
    };
    let root = derive_path::crate_path(&input, "bundle", "pi_ecs_core", Some("core"))?;
    let bundle = quote!(#root::bundle::Bundle);
    let component = quote!(#root::component::Component);
    let plain = quote!(#root::component::PlainAccess);
    let mut generics = input.generics;
    let mut types = Vec::new();
    let mut validates = Vec::new();
    let mut writes = Vec::new();
    for (index, field) in data.fields.iter().enumerate() {
        let member = field
            .ident
            .clone()
            .map_or_else(|| syn::Member::Unnamed(index.into()), syn::Member::Named);
        let ty = &field.ty;
        let span = ty.span();
        if nested(field)? {
            generics
                .make_where_clause()
                .predicates
                .push(syn::parse_quote_spanned!(span=> #ty: #bundle));
            types.push(quote_spanned!(span=> types.extend(<#ty as #bundle>::component_types());));
            validates
                .push(quote_spanned!(span=> <#ty as #bundle>::validate(&self.#member, context)?;));
            writes.push(quote_spanned!(span=> <#ty as #bundle>::write(self.#member, writer);));
        } else {
            generics
                .make_where_clause()
                .predicates
                .push(syn::parse_quote_spanned!(span=> #ty: #component<Access = #plain>));
            types.push(quote_spanned!(span=> types.push(::std::any::TypeId::of::<#ty>());));
            writes.push(quote_spanned!(span=> writer.write(self.#member);));
        }
    }
    let name = &input.ident;
    let shape_args = generics.params.iter().map(|parameter| match parameter {
        syn::GenericParam::Lifetime(_) => quote!('static),
        syn::GenericParam::Type(parameter) => {
            let ident = &parameter.ident;
            quote!(#ident)
        }
        syn::GenericParam::Const(parameter) => {
            let ident = &parameter.ident;
            quote!(#ident)
        }
    });
    let shape = if generics.params.is_empty() {
        quote!(#name)
    } else {
        quote!(#name <#(#shape_args),*>)
    };
    let mut system_generics = generics.clone();
    for parameter in system_generics.type_params_mut() {
        parameter.bounds.push(syn::parse_quote!('static));
    }
    let (system_impl, _, system_where) = system_generics.split_for_impl();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #bundle for #name #ty_generics #where_clause {
            fn component_types() -> ::std::vec::Vec<::std::any::TypeId> {
                let mut types = ::std::vec::Vec::new();
                #(#types)*
                types
            }
            fn validate(&self, context: &#root::bundle::BundleContext<'_>) -> ::std::result::Result<(), #root::error::EcsError> {
                #(#validates)*
                Ok(())
            }
            fn write(self, writer: &mut #root::bundle::BundleWriter<'_>) {
                #(#writes)*
            }
        }
        impl #system_impl #root::bundle::SystemBundle for #name #ty_generics #system_where {
            type Shape = #shape;
        }
    })
}

fn nested(field: &syn::Field) -> syn::Result<bool> {
    let mut nested = false;
    for attr in field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("bundle"))
    {
        attr.parse_args_with(|input: syn::parse::ParseStream<'_>| {
            let key: syn::Ident = input.parse()?;
            if key != "nested" || !input.is_empty() {
                return Err(syn::Error::new(key.span(), "expected #[bundle(nested)]"));
            }
            if nested {
                return Err(syn::Error::new(key.span(), "duplicate nested attribute"));
            }
            nested = true;
            Ok(())
        })?;
    }
    Ok(nested)
}
