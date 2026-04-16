//! Implementation of the `#[lucy_ws(...)]` attribute macro.
//!
//! Parses the attribute arguments into [`WsArgs`] and emits the annotated
//! function along with an `inventory::submit!` block that registers the
//! endpoint in the global [`EndpointRegistry`] at link time.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Ident, ItemFn, LitStr, Token,
};

/// Parsed arguments for the `#[lucy_ws(...)]` attribute.
pub struct WsArgs {
    /// WebSocket upgrade path, e.g. `"/ws/events"`.
    pub path: String,
    /// Optional human-readable description of the endpoint.
    pub description: Option<String>,
}

impl Parse for WsArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Accumulators for each supported key; `Option` lets us detect missing
        // required keys and duplicate assignments.
        let mut path: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;

        // Parse a comma-separated list of `key = "value"` pairs.
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            let _eq: Token![=] = input.parse()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "path" => {
                    if path.is_some() {
                        return Err(syn::Error::new_spanned(
                            &key,
                            "duplicate `path` argument",
                        ));
                    }
                    path = Some(value);
                }
                "description" => {
                    if description.is_some() {
                        return Err(syn::Error::new_spanned(
                            &key,
                            "duplicate `description` argument",
                        ));
                    }
                    description = Some(value);
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        &key,
                        format!(
                            "unknown argument `{other}`; expected one of: path, description"
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

        let path = path
            .ok_or_else(|| syn::Error::new(input.span(), "missing required `path` argument"))?;

        Ok(WsArgs {
            path: path.value(),
            description: description.map(|d| d.value()),
        })
    }
}

/// Expands the `#[lucy_ws(...)]` attribute.
///
/// Parses the attribute arguments, validates them, and emits the original
/// function together with an `inventory::submit!` block that registers the
/// endpoint metadata at link time.
pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as WsArgs);
    let func = parse_macro_input!(item as ItemFn);

    let fn_name = func.sig.ident.to_string();
    let path    = &args.path;

    let description_tokens = match &args.description {
        Some(desc) => quote! { ::core::option::Option::Some(#desc) },
        None => quote! { ::core::option::Option::None },
    };

    let expanded = quote! {
        #func

        ::lucy::_private::inventory::submit! {
            ::lucy::_private::lucy_types::endpoint::EndpointMetaStatic {
                name:        #fn_name,
                path:        #path,
                protocol:    ::lucy::_private::lucy_types::endpoint::Protocol::WebSocket,
                description: #description_tokens,
                method:      ::core::option::Option::None,
            }
        }
    };
    expanded.into()
}
