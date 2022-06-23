#![feature(if_let_guard)]

extern crate proc_macro;
#[macro_use]
extern crate syn;

use std::str::FromStr;

use find_crate::{Dependencies, Manifest};
use proc_macro::{TokenStream};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::{Punctuated, Pair},
    token::{Comma, Colon2},
    Data, DataStruct, DeriveInput, Field, Fields, GenericParam, Ident, Index, LitInt,
    Path, Result, Token, Type, TypePath, PathArguments, GenericArgument, LifetimeDef,
};
use quote::ToTokens;

struct AllTuples {
    macro_ident: Ident,
    start: usize,
    end: usize,
    idents: Vec<Ident>,
}

impl Parse for AllTuples {
    fn parse(input: ParseStream) -> Result<Self> {
        let macro_ident = input.parse::<Ident>()?;
        input.parse::<Comma>()?;
        let start = input.parse::<LitInt>()?.base10_parse()?;
        input.parse::<Comma>()?;
        let end = input.parse::<LitInt>()?.base10_parse()?;
        input.parse::<Comma>()?;
        let mut idents = vec![input.parse::<Ident>()?];
        while input.parse::<Comma>().is_ok() {
            idents.push(input.parse::<Ident>()?);
        }

        Ok(AllTuples {
            macro_ident,
            start,
            end,
            idents,
        })
    }
}

#[proc_macro]
pub fn all_tuples(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as AllTuples);
    let len = input.end + 1;
    let mut ident_tuples = Vec::with_capacity(len);
    for i in 0..len {
        let idents = input
            .idents
            .iter()
            .map(|ident| format_ident!("{}{}", ident, i));
        if input.idents.len() < 2 {
            ident_tuples.push(quote! {
                #(#idents)*
            });
        } else {
            ident_tuples.push(quote! {
                (#(#idents),*)
            });
        }
    }

    let macro_ident = &input.macro_ident;
    let invocations = (input.start..len).map(|i| {
        let ident_tuples = &ident_tuples[0..i];
        quote! {
            #macro_ident!(#(#ident_tuples),*);
        }
    });
    TokenStream::from(quote! {
        #(
            #invocations
        )*
    })
}

static BUNDLE_ATTRIBUTE_NAME: &str = "bundle";


/// 实现组件，重载组件存储
/// example:
/// 	#[derive(Component)]
/// 	#[storage=XXX]
#[proc_macro_derive(Component, attributes(storage))]
pub fn component_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    let gen = impl_component(&ast);
    gen.into()
}

/// 监听，为函数增加监听器属性
/// example: `#[listen(component = (Node, Position, Modify), entity = (Node, Delete))]`
/// example: `#[listen(resource = (Viewport, Modify))]`
#[proc_macro_attribute]
pub fn listen(attr: TokenStream, item: TokenStream) -> TokenStream {
    let gen = impl_listen_component(attr, item);
    gen.into()
}

/// 实现 trait Setup
/// 提供setup方法，用于将某一功能的listener和system全部初始化
/// # example
/// struct AA;
/// impl {
/// 	#[system]
/// 	fn sys1() {}
/// 	#[system]
/// 	fn sys2() {}
/// 	#[listen(component=(Node, Position, Create))]
/// 	fn listen1() {}
/// }
#[proc_macro_attribute]
pub fn setup(attr: TokenStream, item: TokenStream) -> TokenStream {
    let gen = impl_setup(attr, item);
    gen.into()
}

#[proc_macro_attribute]
pub fn setup1(attr: TokenStream, item: TokenStream) -> TokenStream {
	// let ast = syn::parse(item).unwrap();
    // let gen = impl_setup(attr, item);
	let r = quote! {
	};
    r.into()
}

#[derive(Default)]
struct SystemParamFieldAttributes {
    pub ignore: bool,
}

static SYSTEM_PARAM_ATTRIBUTE_NAME: &str = "system_param";

/// Implement `SystemParam` to use a struct as a parameter in a system
#[proc_macro_derive(SystemParam, attributes(system_param))]
pub fn derive_system_param(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("Expected a struct with named fields."),
    };
    let path = pi_ecs_path();

    let field_attributes = fields
        .iter()
        .map(|field| {
            (
                field,
                field
                    .attrs
                    .iter()
                    .find(|a| *a.path.get_ident().as_ref().unwrap() == SYSTEM_PARAM_ATTRIBUTE_NAME)
                    .map_or_else(SystemParamFieldAttributes::default, |a| {
                        syn::custom_keyword!(ignore);
                        let mut attributes = SystemParamFieldAttributes::default();
                        a.parse_args_with(|input: ParseStream| {
                            if input.parse::<Option<ignore>>()?.is_some() {
                                attributes.ignore = true;
                            }
                            Ok(())
                        })
                        .expect("Invalid 'render_resources' attribute format.");

                        attributes
                    }),
            )
        })
        .collect::<Vec<(&Field, SystemParamFieldAttributes)>>();
    let mut fields = Vec::new();
    let mut field_indices = Vec::new();
    let mut field_types = Vec::new();
    let mut ignored_fields = Vec::new();
    let mut ignored_field_types = Vec::new();
    for (i, (field, attrs)) in field_attributes.iter().enumerate() {
        if attrs.ignore {
            ignored_fields.push(field.ident.as_ref().unwrap());
            ignored_field_types.push(&field.ty);
        } else {
            fields.push(field.ident.as_ref().unwrap());
            field_types.push(&field.ty);
            field_indices.push(Index::from(i));
        }
    }

    let generics = ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let lifetimeless_generics: Vec<_> = generics
        .params
        .iter()
        .filter(|g| matches!(g, GenericParam::Type(_)))
        .collect();
	let mut lifetime_generics: Vec<_> = generics
		.params
		.iter()
		.filter(|g| matches!(g, GenericParam::Lifetime(_)))
		.collect();
	let lifetime_generic = if lifetime_generics.len() > 1 || lifetime_generics.len() == 0 {
		// 生命周期参数大于一个，不能自动实现SystemParam
		panic!("lifetime generic count greater than one or is zero");
	} else {
		lifetime_generics.pop().unwrap()
	};
	let generics1 = &generics.params;


    let mut punctuated_generics = Punctuated::<_, Token![,]>::new();
    punctuated_generics.extend(lifetimeless_generics.iter());

    let mut punctuated_generic_idents = Punctuated::<_, Token![,]>::new();
    punctuated_generic_idents.extend(lifetimeless_generics.iter().map(|g| match g {
        GenericParam::Type(g) => &g.ident,
        _ => panic!(),
    }));

    let struct_name = &ast.ident;
    let fetch_struct_name = Ident::new(&format!("{}State", struct_name), Span::call_site());
    let fetch_struct_visibility = &ast.vis;

    TokenStream::from(quote! {
        impl #impl_generics #path::sys::param::SystemParam for #struct_name#ty_generics #where_clause {
            type Fetch = #fetch_struct_name <(#(<#field_types as #path::sys::param::SystemParam>::Fetch,)*), #punctuated_generic_idents>;
        }

        #fetch_struct_visibility struct #fetch_struct_name<TSystemParamState, #punctuated_generic_idents> {
            state: TSystemParamState,
            marker: std::marker::PhantomData<(#punctuated_generic_idents)>
        }

        unsafe impl<TSystemParamState: #path::sys::param::SystemParamState, #punctuated_generics> #path::sys::param::SystemParamState for #fetch_struct_name<TSystemParamState, #punctuated_generic_idents> {
            type Config = TSystemParamState::Config;
            fn init(world: &mut #path::prelude::World, system_state: &mut #path::prelude::SystemState, config: Self::Config) -> Self {
                Self {
                    state: TSystemParamState::init(world, system_state, config),
                    marker: std::marker::PhantomData,
                }
            }

            fn default_config() -> TSystemParamState::Config {
                TSystemParamState::default_config()
            }

            fn apply(&mut self, world: &mut #path::prelude::World) {
                self.state.apply(world)
            }
        }

        impl <'w, #generics1> #path::sys::param::SystemParamFetch<'w, #lifetime_generic> for #fetch_struct_name <(#(<#field_types as #path::sys::param::SystemParam>::Fetch,)*), #punctuated_generic_idents> {
            type Item = #struct_name#ty_generics;
            unsafe fn get_param(
                state: &#lifetime_generic mut Self,
                system_state: &#lifetime_generic #path::sys::system::SystemState,
                world: &#lifetime_generic #path::world::World,
                change_tick: u32,
            ) -> Self::Item {
                #struct_name {
                    #(#fields: <<#field_types as #path::sys::param::SystemParam>::Fetch as #path::sys::param::SystemParamFetch>::get_param(&mut state.state.#field_indices, system_state, world, change_tick),)*
                    #(#ignored_fields: <#ignored_field_types>::default(),)*
                }
            }
        }
    })
}

fn impl_component(ast: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let storage = ast
        .attrs
        .iter()
        .find(|attr| attr.path.segments[0].ident == "storage")
        .map(|attr| {
            syn::parse2::<StorageAttribute>(attr.tokens.clone())
                .unwrap()
                .storage
        });
	// 如果没有指定存储容器，没有必要重载实现
	let storage = match storage {
		Some(r) => r,
		None => return quote! {}
	};

    quote! {
        impl #impl_generics pi_ecs::component::Component for #name #ty_generics #where_clause {
            type Storage = #storage;
        }
    }
}

struct StorageAttribute {
    storage: Path,
}

impl Parse for StorageAttribute {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        let _parenthesized_token = parenthesized!(content in input);

        Ok(StorageAttribute {
            storage: content.parse()?,
        })
    }
}

fn impl_listen_component(attr: TokenStream, item: TokenStream) -> proc_macro2::TokenStream {

	let s = attr.to_string();
	
	let r = String::from("XX<") + s.as_str() + ">";
	
	let args = match TokenStream::from_str(r.as_str()) {
		Ok(r) if let Ok(mut v) = syn::parse::<TypePath>(r.clone()) => {
			v.path.segments.pop().unwrap()
		}
		_ => panic!("impl_listen err, expect `listen(Compnent(A, C, T))`,  fond `listen({})` {}, {}", attr.to_string(), r, s),
	};

	let argments = match args {
		Pair::Punctuated(t,_) => t.arguments,
		Pair::End(t) => t.arguments,
	};

	let mut args = match argments {
		PathArguments::AngleBracketed(r) => r.args,
		_ => panic!("!PathArguments::AngleBracketed"),
	};
	
	let mut list = Vec::new();
	for elem in &mut args {
		if let GenericArgument::Binding(binding) = elem {
			// if let Type::Paren(expr)
			let key = binding.ident.to_string();
			let mut r = String::from("");
			if key=="component" {
				r += "pi_ecs::monitor::ComponentListen";
			} else if key=="resource" {
				r += "pi_ecs::monitor::ResourceListen";
			} else if key=="entity" {
				r += "pi_ecs::monitor::EntityListen";
			} else {
				panic!("!Component | Resource | EntityListen, is{:?}", key);
			}
			

			let r = TokenStream::from_str(r.as_str()).unwrap();
			let p = syn::parse::<Path>(r).unwrap();

			if let Type::Tuple(r) = &mut binding.ty {
				if let Some(last) = r.elems.last_mut() {
					match last {
						Type::Tuple(t) => {
							for e in &mut t.elems {
								if let Type::Path(p) = e {
									let pp = &p.path;
									let mut path = quote!{#pp}.to_string();
									if path == "Create" || path == "Modify" || path == "Delete"{
										path = String::from("pi_ecs::monitor::") + path.as_str();
									}
									p.path = syn::parse::<Path>(TokenStream::from_str(path.as_str()).unwrap()).unwrap();
								} else {
									panic!("is not path:{:?}", quote!{#e}.to_string());
								}
							}
						},
						Type::Path(p) => {
							let pp = &p.path;
							let mut path = quote!{#pp}.to_string();
							if path == "Create" || path == "Modify" || path == "Delete"{
								path = String::from("pi_ecs::monitor::") + path.as_str();
							}
							p.path = syn::parse::<Path>(TokenStream::from_str(path.as_str()).unwrap()).unwrap();
						},
						_ => panic!("event must is Tuple or Path")
					}
				}
				list.push(ListenItem(p, &r.elems));
			} else {
				let ty = &binding.ty;
				panic!("!TypeTuple, {:?}", quote!{#ty}.to_string());
			}
			
			continue;
		}
		panic!("!GenericArgument::Binding");
	}

	let mut f = match syn::parse::<syn::ItemFn>(item) {
		Ok(r) => r,
		Err(_) =>  panic!("impl_listen err: `${:?}`", attr.to_string())
	};

	// panic!("f:{:?}", f);

	if list.len() > 0 && f.sig.inputs.len() >= 1 {
		// return quote! {type ___ =pi_ecs::monitor::Listen<(#(#list)*)>;};
		let r = TokenStream::from(quote! {___:pi_ecs::monitor::Listen<(#(#list,)*)>});
		let i;
		if f.sig.inputs.len() > 1 {
			i = std::mem::replace(&mut f.sig.inputs[1], syn::parse::<syn::FnArg>(r).unwrap());
		} else {
			i = syn::parse::<syn::FnArg>(r).unwrap();
		}
		// panic!("xxx");
		f.sig.inputs.push(i);
	}
	quote! {
		#f
	}
}

fn impl_setup(_attr: TokenStream, item: TokenStream) -> proc_macro2::TokenStream {

	let mut f = match syn::parse::<syn::ItemImpl>(item) {
		Ok(r) => r,
		Err(e) => panic!("setup must declare on impl, {:?}", e),
	};

	let self_type1 = &f.self_ty;
	let mut self_type = GenericsCall(&f.self_ty);
	// panic!("self_type==={:?}", quote! {self_type1});

	let mut runfn = Vec::new();
	let mut initfn = Vec::new();
	let mut listenfn = Vec::new();
	for i in f.items.iter_mut() {
		if let syn::ImplItem::Method(m) = i {
			for i in 0..m.attrs.len() {
				let a = &m.attrs[i];
				let path = &a.path;
				let path = quote! {#path}.to_string();
				if path.ends_with("system") {
					let fn_name = &m.sig.ident;
					runfn.push(quote!{
						stage_builder.add_node(pi_ecs::prelude::IntoSystem::system(#self_type::#fn_name, world));
					});
					m.attrs.remove(i);
					continue;
				} else if path.ends_with("listen") {
					let fn_name = &m.sig.ident;
					listenfn.push(quote!{
						pi_ecs::prelude::ListenSetup::setup(
							pi_ecs::monitor::Listeners::listeners(&#self_type::#fn_name),
							world
						);
					});
					continue;
				} else if path.ends_with("init"){
					let fn_name = &m.sig.ident;
					initfn.push(quote!{
						pi_ecs::prelude::System::run(&mut pi_ecs::prelude::IntoSystem::system(#self_type ::#fn_name, world), ());
					});
					m.attrs.remove(i);
					continue;
				}
			}
		}
	}

	let generics = &f.generics;
    let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();
	let r = quote! {
		#f
		// unsafe impl #impl_generics #ecs_path::bundle::Bundle for #struct_name #ty_generics #where_clause {
		impl #impl_generics pi_ecs::prelude::Setup for #self_type1 #where_clause {
			fn setup(world: &mut pi_ecs::prelude::World, stage_builder: &mut pi_ecs::prelude::StageBuilder) {
				#(#initfn)*
				#(#listenfn)*
				#(#runfn)*
			}
		}
	};
	// panic!("====={:?}", r.to_string());
	r
}

struct GenericsCall<'a>(&'a Box<Type>);

impl<'a> ToTokens for GenericsCall<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
		let ty = self.0;

		match &**ty {
			Type::Path(path) => {
				let path = &path.path;
				let arguments = &path.segments.last().unwrap().arguments;

				if let PathArguments::None = arguments {
					tokens.extend(quote! {#path})
				} else {
					let mut self_type = path.clone();
					self_type.segments.last_mut().unwrap().arguments = PathArguments::None;
					tokens.extend(quote! {#self_type::#arguments})
				}
			},
			_ => panic!("self_type is not path"),
		}

		
	}
}

struct ListenItem<'a>(Path, &'a syn::punctuated::Punctuated<Type, Comma>);
impl<'a> ToTokens for ListenItem<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
		let (name, fileds) = (&self.0, &self.1);
		tokens.extend(quote! {#name<#fileds>});
	}
}

// #[listen(
// 	Component(Node, C, (CreateEvent, DeleteEvent)), 
// 	Resource(R, (CreateEvent, DeleteEvent)), 
// 	Entity(A,(CreateEvent, DeleteEvent))
// )]
// fn aa(e: Event, _listen: Listen<ComponentListen<Node, C, (CreateEvent, DeleteEvent)>>) {

// }

#[proc_macro_derive(Bundle, attributes(bundle))]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ecs_path = pi_ecs_path();

    let named_fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("Expected a struct with named fields."),
    };

    let is_bundle = named_fields
        .iter()
        .map(|field| {
            field
                .attrs
                .iter()
                .any(|a| *a.path.get_ident().as_ref().unwrap() == BUNDLE_ATTRIBUTE_NAME)
        })
        .collect::<Vec<bool>>();
    let field = named_fields
        .iter()
        .map(|field| field.ident.as_ref().unwrap())
        .collect::<Vec<_>>();
    let field_type = named_fields
        .iter()
        .map(|field| &field.ty)
        .collect::<Vec<_>>();

    let mut field_type_infos = Vec::new();
    let mut field_get_components = Vec::new();
    let mut field_from_components = Vec::new();
    for ((field_type, is_bundle), field) in
        field_type.iter().zip(is_bundle.iter()).zip(field.iter())
    {
        if *is_bundle {
            field_type_infos.push(quote! {
                type_info.extend(<#field_type as #ecs_path::bundle::Bundle>::type_info());
            });
            field_get_components.push(quote! {
                self.#field.get_components(&mut func);
            });
            field_from_components.push(quote! {
                #field: <#field_type as #ecs_path::bundle::Bundle>::from_components(&mut func),
            });
        } else {
            field_type_infos.push(quote! {
                type_info.push(#ecs_path::component::TypeInfo::of::<#field_type>());
            });
            field_get_components.push(quote! {
                func((&mut self.#field as *mut #field_type).cast::<u8>());
                std::mem::forget(self.#field);
            });
            field_from_components.push(quote! {
                #field: func().cast::<#field_type>().read(),
            });
        }
    }
    let field_len = field.len();
    let generics = ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let struct_name = &ast.ident;

    TokenStream::from(quote! {
        /// SAFE: TypeInfo is returned in field-definition-order. [from_components] and [get_components] use field-definition-order
        unsafe impl #impl_generics #ecs_path::bundle::Bundle for #struct_name #ty_generics #where_clause {
            fn type_info() -> Vec<#ecs_path::component::TypeInfo> {
                let mut type_info = Vec::with_capacity(#field_len);
                #(#field_type_infos)*
                type_info
            }

            #[allow(unused_variables, unused_mut, non_snake_case)]
            unsafe fn from_components(mut func: impl FnMut() -> *mut u8) -> Self {
                Self {
                    #(#field_from_components)*
                }
            }

            #[allow(unused_variables, unused_mut, forget_copy, forget_ref)]
            fn get_components(mut self, mut func: impl FnMut(*mut u8)) {
                #(#field_get_components)*
            }
        }
    })
}

fn get_idents(fmt_string: fn(usize) -> String, count: usize) -> Vec<Ident> {
    (0..count)
        .map(|i| Ident::new(&fmt_string(i), Span::call_site()))
        .collect::<Vec<Ident>>()
}

// fn get_lifetimes(fmt_string: fn(usize) -> String, count: usize) -> Vec<Lifetime> {
//     (0..count)
//         .map(|i| Lifetime::new(&fmt_string(i), Span::call_site()))
//         .collect::<Vec<Lifetime>>()
// }

struct TupleRang {
    start: usize,
    end: usize,
}

impl Parse for TupleRang {
    fn parse(input: ParseStream) -> Result<Self> {
        let start = input.parse::<LitInt>()?.base10_parse()?;
        input.parse::<Comma>()?;
        let end = input.parse::<LitInt>()?.base10_parse()?;

        Ok(TupleRang {
            start,
            end,
        })
    }
}


#[proc_macro]
pub fn impl_param_set_field(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as TupleRang);
	let max_queries = input.end + 1;

    let mut tokens = TokenStream::new();
    let queries = get_idents(|i| format!("P{}", i), max_queries);

    let mut query_fns = Vec::new();
    let mut query_fn_muts = Vec::new();
    for i in 0..max_queries {
        let query = &queries[i];
        let fn_name = Ident::new(&format!("p{}", i), Span::call_site());
        let fn_name_mut = Ident::new(&format!("p{}_mut", i), Span::call_site());
        let index = Index::from(i);
        query_fns.push(quote! {
            pub fn #fn_name(&self) -> &#query{
                &self.0.#index
            }
        });
        query_fn_muts.push(quote! {
            pub fn #fn_name_mut(&mut self) -> &mut #query {
                &mut self.0.#index
            }
        });
    }

    for query_count in input.start..max_queries {
        let query = &queries[0..query_count];
        let query_fn = &query_fns[0..query_count];
        let query_fn_mut = &query_fn_muts[0..query_count];
        tokens.extend(TokenStream::from(quote! {

            impl<#(#query: SystemParam,)*> ParamSet<(#(#query,)*)> {
                #(#query_fn)*
                #(#query_fn_mut)*
            }
        }));
    }

    tokens
}


#[proc_macro_derive(SystemLabel)]
pub fn derive_system_label(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    derive_label(input, Ident::new("SystemLabel", Span::call_site())).into()
}

#[proc_macro_derive(StageLabel)]
pub fn derive_stage_label(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_label(input, Ident::new("StageLabel", Span::call_site())).into()
}

#[proc_macro_derive(AmbiguitySetLabel)]
pub fn derive_ambiguity_set_label(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_label(input, Ident::new("AmbiguitySetLabel", Span::call_site())).into()
}

#[proc_macro_derive(RunCriteriaLabel)]
pub fn derive_run_criteria_label(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_label(input, Ident::new("RunCriteriaLabel", Span::call_site())).into()
}

fn derive_label(input: DeriveInput, label_type: Ident) -> TokenStream2 {
    let ident = input.ident;
    let ecs_path: Path = pi_ecs_path();

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut where_clause = where_clause.cloned().unwrap_or_else(|| syn::WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });
    where_clause.predicates.push(syn::parse2(quote! { Self: Eq + ::std::fmt::Debug + ::std::hash::Hash + Clone + Send + Sync + 'static }).unwrap());

    quote! {
        impl #impl_generics #ecs_path::schedule::#label_type for #ident #ty_generics #where_clause {
            fn dyn_clone(&self) -> Box<dyn #ecs_path::schedule::#label_type> {
                Box::new(Clone::clone(self))
            }
        }
    }
}

fn pi_ecs_path() -> syn::Path {
    fn find_in_manifest(manifest: &mut Manifest, dependencies: Dependencies) -> Option<String> {
        manifest.dependencies = dependencies;
        // if let Some(package) = manifest.find(|name| name == "bevy") {
        //     Some(format!("{}::ecs", package.name))
        // } else if let Some(package) = manifest.find(|name| name == "bevy_internal") {
        //     Some(format!("{}::ecs", package.name))
        // } else 
		if let Some(package) = manifest.find(|name| name == "bevy_ecs") {
            Some(package.name)
        } else {
            None
        }
    }

    let mut manifest = Manifest::new().unwrap();
    let path_str = find_in_manifest(&mut manifest, Dependencies::Release)
        .or_else(|| find_in_manifest(&mut manifest, Dependencies::Dev))
        .unwrap_or_else(|| "pi_ecs".to_string());

    let path: Path = syn::parse(path_str.parse::<TokenStream>().unwrap()).unwrap();
    path
}
