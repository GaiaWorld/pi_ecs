//! 类型化派生展开和输入诊断。
use proc_macro::TokenStream;

#[cfg(test)]
#[test]
fn named_param_expansion_rebinds_borrow_and_requires_item_equality() {
    let result = expand_with_roots(
        syn::parse_quote!(
            struct Named<'w, T: 'static> {
                local: Local<'w, T>,
            }
        ),
        quote::quote!(schedule),
        quote::quote!(core),
    )
    .unwrap();
    let _: syn::ItemImpl = syn::parse2(result.clone()).unwrap();
    let result = result.to_string();
    assert!(result.contains("Local < 'static , T >"));
    assert!(result.contains("Item < '__ecs_run > = Local < '__ecs_run , T >"));
    assert!(result.contains("Named { local : __ecs_field_0 , }"));
    assert!(expand_with_roots(
        syn::parse_quote!(
            struct Invalid<'a, 'b> {
                value: &'a &'b u32,
            }
        ),
        quote::quote!(schedule),
        quote::quote!(core),
    )
    .is_err());
}
#[path = "derive_path_inner.rs"]
mod derive_path;

pub(super) fn derive_system_param_inner(input: TokenStream) -> TokenStream {
    syn::parse(input)
        .and_then(|input| {
            let schedule = derive_path::crate_path(
                &input,
                "system_param",
                "pi_ecs_schedule",
                Some("schedule"),
            )?;
            let core = derive_path::crate_path(
                &input,
                "__unused_ecs_core_path",
                "pi_ecs_core",
                Some("core"),
            )?;
            expand_with_roots(input, schedule, core)
        })
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

struct Rebind<'a> {
    from: &'a syn::Lifetime,
    to: &'a syn::Lifetime,
}

impl syn::visit_mut::VisitMut for Rebind<'_> {
    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident == self.from.ident {
            *lifetime = self.to.clone();
        }
    }
}

fn expand_with_roots(
    input: syn::DeriveInput,
    schedule: proc_macro2::TokenStream,
    core: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    use quote::{format_ident, quote};
    use syn::visit_mut::VisitMut;
    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "SystemParam requires a named struct",
        ));
    };
    let syn::Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "SystemParam requires named fields",
        ));
    };
    if fields.named.len() > 8 {
        return Err(syn::Error::new_spanned(
            fields,
            "SystemParam supports at most eight fields",
        ));
    }
    let mut lifetimes = input.generics.lifetimes();
    let Some(declared) = lifetimes.next() else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "expected one run lifetime",
        ));
    };
    if lifetimes.next().is_some() || !declared.bounds.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "expected one unbounded run lifetime",
        ));
    }
    let declared = &declared.lifetime;
    let run_name = if declared.ident == "__ecs_run" {
        "'__ecs_run_"
    } else {
        "'__ecs_run"
    };
    let run = syn::Lifetime::new(run_name, proc_macro2::Span::mixed_site());
    let static_lifetime = syn::parse_quote!('static);
    let mut generics = input.generics.clone();
    let param = quote!(#schedule::system_param::SystemParam);
    let reg = quote!(#schedule::system_param::ParamRegistration);
    let ctx = quote!(#schedule::system::SystemRunContext);
    let error = quote!(#core::error::EcsError);
    let mut descriptions = Vec::new();
    let mut names = Vec::new();
    for field in &fields.named {
        derive_path::reject_attributes(&field.attrs, "system_param")?;
        let mut description = field.ty.clone();
        let mut item = field.ty.clone();
        Rebind {
            from: declared,
            to: &static_lifetime,
        }
        .visit_type_mut(&mut description);
        Rebind {
            from: declared,
            to: &run,
        }
        .visit_type_mut(&mut item);
        generics
            .make_where_clause()
            .predicates
            .push(syn::parse_quote!(#description: for<#run> #param<Item<#run> = #item>));
        descriptions.push(description);
        names.push(field.ident.as_ref().expect("named field"));
    }
    let locals = (0..names.len())
        .map(|i| format_ident!("__ecs_field_{i}"))
        .collect::<Vec<_>>();
    let args = input.generics.params.iter().map(|param| match param {
        syn::GenericParam::Lifetime(_) => quote!(#run),
        syn::GenericParam::Type(param) => {
            let id = &param.ident;
            quote!(#id)
        }
        syn::GenericParam::Const(param) => {
            let id = &param.ident;
            quote!(#id)
        }
    });
    let name = &input.ident;
    let (_, original, _) = input.generics.split_for_impl();
    let (implementation, _, where_clause) = generics.split_for_impl();
    let tuple = quote!((#(#descriptions,)*));
    Ok(quote! {
        unsafe impl #implementation #param for #name #original #where_clause {
            type Config = <#tuple as #param>::Config;
            type State = <#tuple as #param>::State;
            type Pending = <#tuple as #param>::Pending;
            type Item<#run> = #name<#(#args),*>;
            fn initialize(config: Self::Config, registration: &mut #reg<'_>) -> Result<Self::State, #error> {
                <#tuple as #param>::initialize(config, registration)
            }
            fn prepare(state: &mut Self::State, world: #core::world::WorldRef<'_>) -> Result<#schedule::param_composition::ParamAccess, #error> {
                <#tuple as #param>::prepare(state, world)
            }
            fn declaration(state: &Self::State, world: #core::world::WorldRef<'_>) -> Result<#core::access::AccessTree, #error> {
                <#tuple as #param>::declaration(state, world)
            }
            fn ordering_access(state: &Self::State, logical: &#core::access::OrderingArchetypes) -> Result<Vec<#core::access::OrderingAccess>, #error> {
                <#tuple as #param>::ordering_access(state, logical)
            }
            fn begin(state: &mut Self::State) -> Self::Pending {
                <#tuple as #param>::begin(state)
            }
            unsafe fn fetch<#run>(state: &#run mut Self::State, pending: &#run mut Self::Pending, context: #ctx<#run>) -> Self::Item<#run> {
                let (#(#locals,)*) = unsafe { <#tuple as #param>::fetch(state, pending, context) };
                #name { #(#names: #locals,)* }
            }
            fn finish(state: &mut Self::State, pending: Self::Pending, context: &#ctx<'_>) {
                <#tuple as #param>::finish(state, pending, context)
            }
            fn unregister(state: Self::State, registration: &mut #reg<'_>) -> Result<(), #error> {
                <#tuple as #param>::unregister(state, registration)
            }
        }
    })
}
