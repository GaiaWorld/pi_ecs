pub(crate) fn reject_attributes(attrs: &[syn::Attribute], helper: &str) -> syn::Result<()> {
    if let Some(attr) = attrs.iter().find(|attr| attr.path().is_ident(helper)) {
        return Err(syn::Error::new_spanned(
            attr,
            "attribute is not allowed here",
        ));
    }
    Ok(())
}

pub(crate) fn crate_path(
    input: &syn::DeriveInput,
    helper: &str,
    direct: &str,
    facade_module: Option<&str>,
) -> syn::Result<proc_macro2::TokenStream> {
    use proc_macro_crate::crate_name;
    let mut explicit = None;
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident(helper))
    {
        let mut present = false;
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("crate") {
                return Err(meta.error("expected crate = path"));
            }
            if explicit.is_some() {
                return Err(meta.error("duplicate crate path"));
            }
            explicit = Some(meta.value()?.parse::<syn::Path>()?);
            present = true;
            Ok(())
        })?;
        if !present {
            return Err(syn::Error::new_spanned(attr, "expected crate = path"));
        }
    }
    if let Some(path) = explicit {
        return Ok(quote::quote!(#path));
    }
    if let Ok(found) = crate_name(direct) {
        return Ok(root_tokens(found));
    }
    if let (Ok(found), Some(module)) = (crate_name("pi_ecs"), facade_module) {
        let root = root_tokens(found);
        let module = syn::Ident::new(module, proc_macro2::Span::call_site());
        return Ok(quote::quote!(#root::#module));
    }
    Err(syn::Error::new_spanned(
        &input.ident,
        format!("cannot resolve {direct}; use #[{helper}(crate = path)]"),
    ))
}

fn root_tokens(found: proc_macro_crate::FoundCrate) -> proc_macro2::TokenStream {
    match found {
        proc_macro_crate::FoundCrate::Itself => quote::quote!(crate),
        proc_macro_crate::FoundCrate::Name(name) => {
            let name = syn::Ident::new(&name, proc_macro2::Span::call_site());
            quote::quote!(::#name)
        }
    }
}
