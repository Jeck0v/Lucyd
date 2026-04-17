//! Implementation of the `#[lucy_http(...)]` attribute macro.
//!
//! Parses the attribute arguments into [`HttpArgs`] and emits the annotated
//! function along with an `inventory::submit!` block that registers the
//! endpoint in the global [`EndpointRegistry`] at link time.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Ident, ItemFn, LitStr, Token,
};

/// Parsed arguments for the `#[lucy_http(...)]` attribute.
pub struct HttpArgs {
    /// HTTP verb as written by the user (e.g. `"GET"`, `"POST"`).
    pub method: String,
    /// URL path, e.g. `"/api/users"`.
    pub path: String,
    /// Optional human-readable description of the endpoint.
    pub description: Option<String>,
    /// Optional comma-separated tags for grouping in the documentation UI.
    pub tags: Vec<String>,
    /// Optional request body type for JSON Schema generation.
    pub request_type: Option<syn::Type>,
    /// Optional response body type for JSON Schema generation.
    pub response_type: Option<syn::Type>,
}

impl Parse for HttpArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Accumulators for each supported key. We keep them as `Option` so we
        // can detect both missing required keys and duplicate assignments.
        let mut method: Option<LitStr> = None;
        let mut path: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;
        let mut tags: Option<LitStr> = None;
        let mut request_type: Option<syn::Type> = None;
        let mut response_type: Option<syn::Type> = None;

        // Parse a comma-separated list of `key = value` pairs.
        // String arguments use `"value"`, while `request` and `response`
        // accept a bare type path (e.g. `MyStruct`).
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            let _eq: Token![=] = input.parse()?;

            match key.to_string().as_str() {
                "method" => {
                    if method.is_some() {
                        return Err(syn::Error::new_spanned(&key, "duplicate `method` argument"));
                    }
                    method = Some(input.parse::<LitStr>()?);
                }
                "path" => {
                    if path.is_some() {
                        return Err(syn::Error::new_spanned(&key, "duplicate `path` argument"));
                    }
                    path = Some(input.parse::<LitStr>()?);
                }
                "description" => {
                    if description.is_some() {
                        return Err(syn::Error::new_spanned(
                            &key,
                            "duplicate `description` argument",
                        ));
                    }
                    description = Some(input.parse::<LitStr>()?);
                }
                "tags" => {
                    if tags.is_some() {
                        return Err(syn::Error::new_spanned(&key, "duplicate `tags` argument"));
                    }
                    tags = Some(input.parse::<LitStr>()?);
                }
                "request" => {
                    if request_type.is_some() {
                        return Err(syn::Error::new_spanned(
                            &key,
                            "duplicate `request` argument",
                        ));
                    }
                    request_type = Some(input.parse::<syn::Type>()?);
                }
                "response" => {
                    if response_type.is_some() {
                        return Err(syn::Error::new_spanned(
                            &key,
                            "duplicate `response` argument",
                        ));
                    }
                    response_type = Some(input.parse::<syn::Type>()?);
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        &key,
                        format!(
                            "unknown argument `{other}`; expected one of: method, path, description, tags, request, response"
                        ),
                    ));
                }
            }

            // Consume a trailing comma if present; otherwise we're done.
            if input.is_empty() {
                break;
            }
            let _comma: Token![,] = input.parse()?;
        }

        let method = method
            .ok_or_else(|| syn::Error::new(input.span(), "missing required `method` argument"))?;
        let path =
            path.ok_or_else(|| syn::Error::new(input.span(), "missing required `path` argument"))?;

        let tags_vec: Vec<String> = tags
            .map(|t| {
                t.value()
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        Ok(HttpArgs {
            method: method.value(),
            path: path.value(),
            description: description.map(|d| d.value()),
            tags: tags_vec,
            request_type,
            response_type,
        })
    }
}

/// Expands the `#[lucy_http(...)]` attribute.
///
/// Parses the attribute arguments, validates them, and emits the original
/// function together with an `inventory::submit!` block that registers the
/// endpoint metadata at link time.
/// Generates tokens for a schema fn pointer field (`request_schema_fn` or
/// `response_schema_fn`).  When the user supplied a type, we emit a closure
/// that calls `schemars::schema_for!` at runtime; otherwise we emit `None`.
fn schema_fn_tokens(ty: Option<&syn::Type>) -> proc_macro2::TokenStream {
    match ty {
        Some(t) => quote! {
            ::core::option::Option::Some(|| {
                ::lucyd::_private::serde_json::to_value(
                    ::lucyd::_private::schemars::schema_for!(#t)
                ).unwrap_or(::lucyd::_private::serde_json::Value::Null)
            })
        },
        None => quote! { ::core::option::Option::None },
    }
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as HttpArgs);
    let func = parse_macro_input!(item as ItemFn);

    let fn_name = func.sig.ident.to_string();
    let path = &args.path;
    let method = &args.method;

    let description_tokens = match &args.description {
        Some(desc) => quote! { ::core::option::Option::Some(#desc) },
        None => quote! { ::core::option::Option::None },
    };

    let tag_lits: Vec<LitStr> = args
        .tags
        .iter()
        .map(|t| LitStr::new(t, Span::call_site()))
        .collect();
    let tags_tokens = if tag_lits.is_empty() {
        quote! { &[] }
    } else {
        quote! { &[#(#tag_lits),*] }
    };

    let request_schema_tokens = schema_fn_tokens(args.request_type.as_ref());
    let response_schema_tokens = schema_fn_tokens(args.response_type.as_ref());

    let expanded = quote! {
        #func

        ::lucyd::_private::inventory::submit! {
            ::lucyd::_private::lucy_types::endpoint::EndpointMetaStatic {
                name:              #fn_name,
                path:              #path,
                protocol:          ::lucyd::_private::lucy_types::endpoint::Protocol::Http,
                description:       #description_tokens,
                method:            ::core::option::Option::Some(#method),
                tags:              #tags_tokens,
                request_schema_fn:  #request_schema_tokens,
                response_schema_fn: #response_schema_tokens,
            }
        }
    };
    expanded.into()
}
