//! 将具名查询委托给同序字段元组，并将实际取值结果重组为具名字段。
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::{parse_quote, visit_mut::VisitMut, DeriveInput, GenericParam, Lifetime};

#[path = "derive_path_inner.rs"]
mod derive_path;

pub(super) fn derive_inner(input: TokenStream) -> TokenStream {
    syn::parse(input)
        .and_then(expand)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Default)]
struct UsedNames(std::collections::HashSet<String>);

impl VisitMut for UsedNames {
    fn visit_ident_mut(&mut self, ident: &mut Ident) {
        self.0.insert(ident.to_string());
    }
}

impl UsedNames {
    fn fresh(&mut self, base: &str) -> Ident {
        let mut name = format!("__ecs_{base}");
        while !self.0.insert(name.clone()) {
            name.push('_');
        }
        Ident::new(&name, Span::mixed_site())
    }

    fn lifetime(&mut self, base: &str) -> Lifetime {
        let ident = self.fresh(base);
        Lifetime::new(&format!("'{ident}"), ident.span())
    }
}

// These shapes must match exactly: field order is part of the access contract.
fn tuple(tokens: &[Tokens]) -> Tokens {
    if tokens.len() <= 8 {
        return quote!((#(#tokens,)*));
    }
    let (head, tail) = tokens.split_at(7);
    let tail = tuple(tail);
    quote!((#(#head,)* #tail,))
}

fn expand(mut input: DeriveInput) -> syn::Result<Tokens> {
    let mut used = UsedNames::default();
    used.visit_derive_input_mut(&mut input);
    let named = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(named) => named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "QueryData requires named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "QueryData requires a struct",
            ))
        }
    };
    let root = derive_path::crate_path(&input, "query_data", "pi_ecs_core", Some("core"))?;
    let data = quote!(#root::query::QueryData);
    let readonly = quote!(#root::query::ReadOnlyQueryData);
    let fetch = quote!(#root::query_fetch::QueryFetch);
    let scope_ty = quote!(#root::query_fetch::FetchScope);
    let result = quote!(::core::result::Result);
    let error = quote!(#root::error::EcsError);
    let name = &input.ident;
    let visibility = &input.vis;
    let item = format_ident!("{name}Item");
    let adapter = used.fresh("fetch_adapter");
    let loan = used.lifetime("loan");
    let world = used.lifetime("world");
    let proof = used.lifetime("proof");
    let [source, state, scope, binding, access, plan, entity, prepared] = [
        "source", "state", "scope", "binding", "access", "plan", "entity", "prepared",
    ]
    .map(|base| used.fresh(base));

    let mut types = Vec::new();
    let mut fields = Vec::new();
    let mut field_vis = Vec::new();
    let mut item_params = Vec::new();
    let mut values = Vec::new();
    let mut generics = input.generics.clone();
    for field in &named.named {
        derive_path::reject_attributes(&field.attrs, "query_data")?;
        let ty = &field.ty;
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: #data));
        types.push(ty);
        fields.push(field.ident.as_ref().expect("named field"));
        field_vis.push(&field.vis);
        item_params.push(used.fresh("item"));
        values.push(used.fresh("value"));
    }
    for param in &generics.params {
        let attrs = match param {
            GenericParam::Lifetime(param) => &param.attrs,
            GenericParam::Type(param) => &param.attrs,
            GenericParam::Const(param) => &param.attrs,
        };
        derive_path::reject_attributes(attrs, "query_data")?;
    }
    let field_tuple = tuple(&types.iter().map(|ty| quote!(#ty)).collect::<Vec<_>>());
    let pattern = tuple(
        &values
            .iter()
            .map(|value| quote!(#value))
            .collect::<Vec<_>>(),
    );
    let (_, target_args, _) = input.generics.split_for_impl();
    let target = quote!(#name #target_args);
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let mut fetch_generics = generics.clone();
    fetch_generics.params.insert(0, parse_quote!(#world));
    let (fetch_impl, fetch_args, _) = fetch_generics.split_for_impl();
    let mut read_generics = generics.clone();
    // A higher-ranked receiver bound lets concrete mutable fields compile as QueryData,
    // while requiring the real tuple to implement ReadOnlyQueryData before shared use.
    read_generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(
            for<#proof> &#proof #field_tuple: __EcsReadOnlyFields
        ));
    let (read_impl, _, read_where) = read_generics.split_for_impl();
    let (item_decl, item_ty) = if fields.is_empty() {
        (quote!(), quote!(#item))
    } else {
        (
            quote!(<#(#item_params),*>),
            quote!(#item<#(<#types as #data>::Item<#loan, #world>),*>),
        )
    };

    Ok(quote! {
        /// 查询实际返回的具名字段。
        #[allow(non_camel_case_types)]
        #visibility struct #item #item_decl {
            #(#field_vis #fields: #item_params,)*
        }

        const _: () = {
            // Local witness: only tuples that already implement the read-only protocol qualify.
            trait __EcsReadOnlyFields {}
            impl<T: #readonly> __EcsReadOnlyFields for &T {}

            #[allow(non_camel_case_types)]
            pub struct #adapter #fetch_impl #where_clause {
                fields: <#field_tuple as #data>::Fetch<#world>,
                marker: ::core::marker::PhantomData<fn() -> #target>,
            }

            // SAFETY: declaration and preparation are exactly those of the same field tuple.
            unsafe impl #impl_generics #data for #target #where_clause {
                type Item<#loan, #world> = #item_ty;
                type State = <#field_tuple as #data>::State;
                type FetchState = <#field_tuple as #data>::FetchState;
                type Fetch<#world> = #adapter #fetch_args;

                fn init_state(#source: &#root::world::World) -> #result<Self::State, #error> {
                    <#field_tuple as #data>::init_state(#source)
                }

                fn declare_access(#state: &Self::State) -> #result<#root::query::QueryDataDeclaration, #error> {
                    <#field_tuple as #data>::declare_access(#state)
                }

                fn init_fetch_state(
                    #state: &Self::State,
                    #scope: #scope_ty,
                    #binding: &#root::query_fetch::FetchBinding<'_>,
                ) -> #result<#root::query::FetchStateResult<Self::FetchState>, #error> {
                    <#field_tuple as #data>::init_fetch_state(#state, #scope, #binding)
                }
            }

            // SAFETY: the witness is implemented only for tuples of read-only fields.
            unsafe impl #read_impl #readonly for #target #read_where {}

            // SAFETY: use the original authorization and the same validated entity exactly once.
            unsafe impl #fetch_impl #fetch<#world, #target> for #adapter #fetch_args #where_clause {
                unsafe fn prepare(
                    #access: &#root::query_fetch::FetchAccess<#world>,
                    #scope: #scope_ty,
                    #state: &<#target as #data>::State,
                    #plan: &<#target as #data>::FetchState,
                ) -> #result<Self, #error> {
                    let #prepared = unsafe {
                        <<#field_tuple as #data>::Fetch<#world> as #fetch<#world, #field_tuple>>::prepare(
                            #access, #scope, #state, #plan,
                        )?
                    };
                    ::core::result::Result::Ok(Self { fields: #prepared, marker: ::core::marker::PhantomData })
                }

                unsafe fn fetch_at(&mut self, #entity: &#root::query_fetch::FetchEntity)
                    -> <#target as #data>::Item<#world, #world>
                {
                    let #pattern = unsafe {
                        <<#field_tuple as #data>::Fetch<#world> as #fetch<#world, #field_tuple>>::fetch_at(
                            &mut self.fields, #entity,
                        )
                    };
                    #item { #(#fields: #values,)* }
                }
            }
        };
    })
}
