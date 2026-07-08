//! Shared parsing and code-generation helpers for the `#[lucy_http]`,
//! `#[lucy_ws]`, and `#[lucy_mqtt]` attribute macros.
//!
//! The three macros share almost all of their attribute-parsing and
//! token-generation logic; only the accepted keys, the `Protocol` variant,
//! and whether a `method` field exists differ between them. This module
//! hoists the identical parts so each macro's file only carries its own
//! protocol-specific wiring.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, LitStr, Token, parse::ParseStream};

/// Parses a `key = <value>` pair into `slot`, erroring if `slot` is already
/// populated (i.e. the key was supplied twice).
///
/// `input` must be positioned right after the `=` token; the value is parsed
/// via `T`'s own [`syn::parse::Parse`] impl.
pub fn parse_unique<T: syn::parse::Parse>(
    key: &Ident,
    slot: &mut Option<T>,
    input: ParseStream,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new_spanned(
            key,
            format!("duplicate `{key}` argument"),
        ));
    }
    *slot = Some(input.parse()?);
    Ok(())
}

/// Builds the error for a key that isn't one of the macro's supported
/// arguments, listing the `allowed` keys in the message.
pub fn unknown_argument_error(key: &Ident, allowed: &str) -> syn::Error {
    syn::Error::new_spanned(
        key,
        format!("unknown argument `{key}`; expected one of: {allowed}"),
    )
}

/// Consumes a trailing comma between `key = value` pairs, if present.
///
/// A missing comma is only valid at the very end of the argument list, which
/// is exactly the case where `input` is already empty.
pub fn consume_trailing_comma(input: ParseStream) -> syn::Result<()> {
    if input.is_empty() {
        return Ok(());
    }
    input.parse::<Token![,]>()?;
    Ok(())
}

/// Requires that a required argument was supplied, erroring with a
/// "missing required" message (pointing at `span`) otherwise.
pub fn require(value: Option<LitStr>, name: &str, span: Span) -> syn::Result<LitStr> {
    value.ok_or_else(|| syn::Error::new(span, format!("missing required `{name}` argument")))
}

/// Splits a comma-separated `tags` literal into a trimmed, non-empty list.
///
/// Absent `tags` (`None`) yields an empty `Vec`.
pub fn parse_tags(tags: Option<LitStr>) -> Vec<String> {
    tags.map(|t| {
        t.value()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    })
    .unwrap_or_default()
}

/// Generates tokens for an `Option<&str>` field (e.g. `description`): `Some(#v)`
/// when present, `None` otherwise.
pub fn option_str_tokens(value: Option<&str>) -> TokenStream {
    match value {
        Some(v) => quote! { ::core::option::Option::Some(#v) },
        None => quote! { ::core::option::Option::None },
    }
}

/// Generates tokens for the `tags: &[&str]` field of `EndpointMetaStatic`.
pub fn tags_tokens(tags: &[String]) -> TokenStream {
    let tag_lits: Vec<LitStr> = tags
        .iter()
        .map(|t| LitStr::new(t, Span::call_site()))
        .collect();
    if tag_lits.is_empty() {
        quote! { &[] }
    } else {
        quote! { &[#(#tag_lits),*] }
    }
}

/// Generates tokens for a schema fn pointer field (`request_schema_fn` or
/// `response_schema_fn`). When the user supplied a type, we emit a closure
/// that calls `schemars::schema_for!` at runtime; otherwise we emit `None`.
pub fn schema_fn_tokens(ty: Option<&syn::Type>) -> TokenStream {
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
