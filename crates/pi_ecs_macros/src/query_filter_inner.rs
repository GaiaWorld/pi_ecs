//! 类型化派生展开和输入诊断。
use proc_macro::TokenStream;

#[cfg(test)]
#[test]
fn wide_filter_preserves_all_fields_in_nested_tuple() {
    let input = syn::parse_quote! {
        #[query_filter(crate = renamed)]
        struct Wide(A, B, C, D, E, F, G, H, I);
    };
    let output = expand(input).unwrap();
    let _: syn::ItemImpl = syn::parse2(output.clone()).unwrap();
    assert!(output
        .to_string()
        .contains("(A , B , C , D , E , F , G , (H , I ,) ,)"));
}
#[cfg(test)]
#[test]
fn filter_expansion_avoids_caller_lifetime_and_result_names() {
    let input = syn::parse_quote! {
        #[query_filter(crate = renamed)]
        struct Named<'__ecs, Result, const world: usize, const __ecs_world: usize>(
            Filter<'__ecs, Result, world>,
            Filter<'__ecs, for<'__ecs_> fn(&'__ecs_ ()), __ecs_world>,
        );
    };
    let item: syn::ItemImpl = syn::parse2(expand(input).unwrap()).unwrap();
    for member in item.items {
        match member {
            syn::ImplItem::Type(ty) if ty.ident == "Fetch" => {
                let name = &ty.generics.lifetimes().next().unwrap().lifetime.ident;
                assert!(name != "__ecs" && name != "__ecs_");
            }
            syn::ImplItem::Fn(method) => {
                for arg in method.sig.inputs {
                    if let syn::FnArg::Typed(arg) = arg {
                        assert!(!matches!(*arg.pat, syn::Pat::Ident(pat)
                            if pat.ident == "world" || pat.ident == "__ecs_world"));
                    }
                }
                let syn::ReturnType::Type(_, ty) = method.sig.output else {
                    panic!("filter methods return a Result");
                };
                let syn::Type::Path(ty) = *ty else {
                    panic!("expected a qualified Result path");
                };
                assert!(ty.path.leading_colon.is_some());
                assert_eq!(ty.path.segments.first().unwrap().ident, "core");
            }
            _ => {}
        }
    }
}

pub(super) fn derive_inner(input: TokenStream) -> TokenStream {
    syn::parse(input)
        .and_then(expand)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[path = "derive_path_inner.rs"]
mod derive_path;

fn tuple(types: &[&syn::Type]) -> proc_macro2::TokenStream {
    use quote::quote;
    if types.len() <= 8 {
        return quote!((#(#types,)*));
    }
    let (head, tail) = types.split_at(7);
    let tail = tuple(tail);
    quote!((#(#head,)* #tail,))
}

// Also avoid lifetimes bound inside field types: those types are reused in the
// generated GAT and method, where inner binders cannot shadow the fetch lifetime.
fn fresh_fetch_lifetime(input: &mut syn::DeriveInput) -> syn::Lifetime {
    use syn::visit_mut::VisitMut;

    #[derive(Default)]
    struct Lifetimes(std::collections::HashSet<String>);

    impl VisitMut for Lifetimes {
        fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
            self.0.insert(lifetime.ident.to_string());
        }
    }

    let mut used = Lifetimes::default();
    used.visit_derive_input_mut(input);
    let mut name = String::from("__ecs");
    while used.0.contains(&name) {
        name.push('_');
    }
    syn::Lifetime::new(&format!("'{name}"), proc_macro2::Span::mixed_site())
}

fn expand(mut input: syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    use quote::quote;
    use syn::spanned::Spanned;
    let fetch_lifetime = fresh_fetch_lifetime(&mut input);
    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "QueryFilter requires a struct",
        ));
    };
    let root = derive_path::crate_path(&input, "query_filter", "pi_ecs_core", Some("core"))?;
    let filter = quote!(#root::query::QueryFilter);
    let parameter = |name: &str| {
        let mut name = format!("__ecs_{name}");
        while input
            .generics
            .const_params()
            .any(|param| param.ident == name)
        {
            name.push('_');
        }
        syn::Ident::new(&name, proc_macro2::Span::mixed_site())
    };
    let [world, state, scope, binding, access, fetch] =
        ["world", "state", "scope", "binding", "access", "fetch"].map(parameter);
    let mut generics = input.generics;
    for param in &generics.params {
        let attrs = match param {
            syn::GenericParam::Lifetime(param) => &param.attrs,
            syn::GenericParam::Type(param) => &param.attrs,
            syn::GenericParam::Const(param) => &param.attrs,
        };
        derive_path::reject_attributes(attrs, "query_filter")?;
    }
    let mut types = Vec::new();
    for field in &data.fields {
        derive_path::reject_attributes(&field.attrs, "query_filter")?;
        let ty = &field.ty;
        generics
            .make_where_clause()
            .predicates
            .push(syn::parse_quote_spanned!(ty.span()=> #ty: #filter));
        types.push(ty);
    }
    let fields = tuple(&types);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    Ok(quote! {
        // SAFETY: every declaration, plan and access delegates to the same field tuple.
        unsafe impl #impl_generics #filter for #name #ty_generics #where_clause {
            const IS_ARCHETYPAL: bool = <#fields as #filter>::IS_ARCHETYPAL;
            type State = <#fields as #filter>::State;
            type FetchState = <#fields as #filter>::FetchState;
            type Fetch<#fetch_lifetime> = <#fields as #filter>::Fetch<#fetch_lifetime>;
            fn init_state(#world: &#root::world::World) -> ::core::result::Result<Self::State, #root::error::EcsError> {
                <#fields as #filter>::init_state(#world)
            }
            fn init_fetch_state(
                #state: &Self::State,
                #scope: #root::query_fetch::FetchScope,
                #binding: &#root::query_fetch::FetchBinding<'_>,
            ) -> ::core::result::Result<#root::query::FetchStateResult<Self::FetchState>, #root::error::EcsError> {
                <#fields as #filter>::init_fetch_state(#state, #scope, #binding)
            }
            fn declare_access(#state: &Self::State) -> ::core::result::Result<#root::query::QueryFilterDeclaration, #root::error::EcsError> {
                <#fields as #filter>::declare_access(#state)
            }
            unsafe fn prepare<#fetch_lifetime>(
                #access: &#root::query_fetch::FetchAccess<#fetch_lifetime>,
                #scope: #root::query_fetch::FetchScope,
                #state: &Self::State,
                #fetch: &Self::FetchState,
            ) -> ::core::result::Result<Self::Fetch<#fetch_lifetime>, #root::error::EcsError> {
                unsafe { <#fields as #filter>::prepare(#access, #scope, #state, #fetch) }
            }
        }
    })
}
