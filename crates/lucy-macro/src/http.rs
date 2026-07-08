//! Implementation of the `#[lucy_http(...)]` attribute macro.
//!
//! Parses the attribute arguments into [`HttpArgs`] and emits the annotated
//! function along with an `inventory::submit!` block that registers the
//! endpoint in the global [`EndpointRegistry`] at link time.

use crate::common;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    Ident, ItemFn, LitStr, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
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

/// Accumulator for `#[lucy_http(...)]` arguments as they're parsed.
///
/// Fields are `Option` so [`Parse::parse`] can detect both missing required
/// keys and duplicate assignments before handing off to [`RawHttpArgs::finalize`].
#[derive(Default)]
struct RawHttpArgs {
    method: Option<LitStr>,
    path: Option<LitStr>,
    description: Option<LitStr>,
    tags: Option<LitStr>,
    request_type: Option<syn::Type>,
    response_type: Option<syn::Type>,
}

impl RawHttpArgs {
    /// Validates required keys and converts raw tokens into [`HttpArgs`].
    fn finalize(self, span: Span) -> syn::Result<HttpArgs> {
        let method = common::require(self.method, "method", span)?;
        let path = common::require(self.path, "path", span)?;

        Ok(HttpArgs {
            method: method.value(),
            path: path.value(),
            description: self.description.map(|d| d.value()),
            tags: common::parse_tags(self.tags),
            request_type: self.request_type,
            response_type: self.response_type,
        })
    }
}

impl Parse for HttpArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut raw = RawHttpArgs::default();

        // Parse a comma-separated list of `key = value` pairs.
        // String arguments use `"value"`, while `request` and `response`
        // accept a bare type path (e.g. `MyStruct`).
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "method" => common::parse_unique(&key, &mut raw.method, input)?,
                "path" => common::parse_unique(&key, &mut raw.path, input)?,
                "description" => common::parse_unique(&key, &mut raw.description, input)?,
                "tags" => common::parse_unique(&key, &mut raw.tags, input)?,
                "request" => common::parse_unique(&key, &mut raw.request_type, input)?,
                "response" => common::parse_unique(&key, &mut raw.response_type, input)?,
                _ => {
                    return Err(common::unknown_argument_error(
                        &key,
                        "method, path, description, tags, request, response",
                    ));
                }
            }

            common::consume_trailing_comma(input)?;
        }

        raw.finalize(input.span())
    }
}

/// Expands the `#[lucy_http(...)]` attribute.
///
/// Parses the attribute arguments, validates them, and emits the original
/// function together with an `inventory::submit!` block that registers the
/// endpoint metadata at link time.
pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as HttpArgs);
    let func = parse_macro_input!(item as ItemFn);

    let fn_name = func.sig.ident.to_string();
    let path = &args.path;
    let method = &args.method;

    let description_tokens = common::option_str_tokens(args.description.as_deref());
    let tags_tokens = common::tags_tokens(&args.tags);
    let request_schema_tokens = common::schema_fn_tokens(args.request_type.as_ref());
    let response_schema_tokens = common::schema_fn_tokens(args.response_type.as_ref());

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
