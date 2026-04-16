//! Implementation of the `#[lucy_http(...)]` attribute macro.
//!
//! Parses the attribute arguments into [`HttpArgs`] and currently emits the
//! annotated function unchanged. The registration side-effect is reserved for
//! a future task once `lucy-core` exposes a stable registration API.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Ident, ItemFn, LitStr, Token,
};

// Identifier of the future registry registration function that the expansion
// will call. Kept as a constant to avoid spreading the literal across sites.
const REGISTRY_FN_IDENT: &str = "lucy_register_http";

/// Parsed arguments for the `#[lucy_http(...)]` attribute.
// Fields are currently consumed only to validate parsing; they will be used
// when the registration side-effect is emitted in a future task.
#[allow(dead_code)]
pub struct HttpArgs {
    /// HTTP verb as written by the user (e.g. `"GET"`, `"POST"`).
    pub method: String,
    /// URL path, e.g. `"/api/users"`.
    pub path: String,
    /// Optional human-readable description of the endpoint.
    pub description: Option<String>,
}

impl Parse for HttpArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Accumulators for each supported key. We keep them as `Option` so we
        // can detect both missing required keys and duplicate assignments.
        let mut method: Option<LitStr> = None;
        let mut path: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;

        // Parse a comma-separated list of `key = "value"` pairs.
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            let _eq: Token![=] = input.parse()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "method" => {
                    if method.is_some() {
                        return Err(syn::Error::new_spanned(
                            &key,
                            "duplicate `method` argument",
                        ));
                    }
                    method = Some(value);
                }
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
                            "unknown argument `{other}`; expected one of: method, path, description"
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

        let method = method.ok_or_else(|| {
            syn::Error::new(input.span(), "missing required `method` argument")
        })?;
        let path = path
            .ok_or_else(|| syn::Error::new(input.span(), "missing required `path` argument"))?;

        Ok(HttpArgs {
            method: method.value(),
            path: path.value(),
            description: description.map(|d| d.value()),
        })
    }
}

/// Expands the `#[lucy_http(...)]` attribute.
///
/// Parses the attribute arguments, validates them, and returns the annotated
/// function unchanged. Registration code will be injected here in a future
/// task via [`REGISTRY_FN_IDENT`].
// TESTME: integration test in xtask
pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _args = parse_macro_input!(attr as HttpArgs);
    let func = parse_macro_input!(item as ItemFn);

    // Reference the constant so the compiler does not warn about it being
    // unused while the registration feature is still pending. Dropping the
    // value is a no-op at runtime.
    let _ = REGISTRY_FN_IDENT;

    // TODO(registration): emit a side-effect call to REGISTRY_FN_IDENT
    // once lucy-core exposes a stable registration API.
    let expanded: TokenStream2 = quote! { #func };
    expanded.into()
}
