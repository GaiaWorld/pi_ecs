//! 类型化派生展开和输入诊断。
use proc_macro::TokenStream;

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn component_expansion_preserves_generics_and_where_clause() {
        let tokens = expand(syn::parse_quote! {
            #[component(crate = renamed::core)]
            struct Example<'a, T, const N: usize>(&'a [T; N]) where T: Copy;
        })
        .unwrap();
        let parsed: syn::ItemImpl = syn::parse2(tokens).unwrap();
        assert_eq!(parsed.generics.params.len(), 3);
        let clause = parsed.generics.where_clause.unwrap();
        assert_eq!(
            quote!(#clause).to_string(),
            "where T : Copy , Self : 'static + :: core :: marker :: Sized + :: core :: marker :: Send + :: core :: marker :: Sync"
        );
        let (_, path, _) = parsed.trait_.unwrap();
        assert_eq!(
            quote!(#path).to_string(),
            "renamed :: core :: component :: Component"
        );
    }

    #[test]
    fn component_rejects_unknown_duplicate_and_field_attributes() {
        for input in [
            syn::parse_quote!(
                #[component(other)]
                struct Invalid;
            ),
            syn::parse_quote!(
                #[component(crate = one, crate = two)]
                struct Invalid;
            ),
            syn::parse_quote!(
                #[component(crate = renamed)]
                struct Invalid(#[component(crate = one)] u8);
            ),
            syn::parse_quote!(
                #[component(crate = renamed)]
                struct Invalid<#[component(crate = one)] T>(T);
            ),
        ] {
            assert!(expand(input).is_err());
        }
    }
}
pub(super) fn derive_component_inner(input: TokenStream) -> TokenStream {
    syn::parse(input)
        .and_then(expand)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(input: syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    for field in fields(&input.data) {
        reject_attributes(&field.attrs, "component")?;
    }
    if let syn::Data::Enum(data) = &input.data {
        for variant in &data.variants {
            reject_attributes(&variant.attrs, "component")?;
        }
    }
    for param in &input.generics.params {
        let attrs = match param {
            syn::GenericParam::Lifetime(param) => &param.attrs,
            syn::GenericParam::Type(param) => &param.attrs,
            syn::GenericParam::Const(param) => &param.attrs,
        };
        reject_attributes(attrs, "component")?;
    }
    let root = crate_path(&input, "component", "pi_ecs_core", Some("core"))?;
    let name = input.ident;
    let mut generics = input.generics;
    generics
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(
            Self: 'static + ::core::marker::Sized + ::core::marker::Send + ::core::marker::Sync
        ));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    Ok(quote::quote! {
        impl #impl_generics #root::component::Component for #name #ty_generics #where_clause {
            type Access = #root::component::PlainAccess;
        }
    })
}

fn fields(data: &syn::Data) -> Box<dyn Iterator<Item = &syn::Field> + '_> {
    match data {
        syn::Data::Struct(data) => Box::new(data.fields.iter()),
        syn::Data::Enum(data) => Box::new(data.variants.iter().flat_map(|variant| &variant.fields)),
        syn::Data::Union(data) => Box::new(data.fields.named.iter()),
    }
}

#[path = "derive_path_inner.rs"]
mod derive_path;
use derive_path::{crate_path, reject_attributes};
