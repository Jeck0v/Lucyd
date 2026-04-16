//! Implementation of the `#[lucy_mqtt(...)]` attribute macro.
//!
//! Parses the attribute arguments into [`MqttArgs`] and currently emits the
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
const REGISTRY_FN_IDENT: &str = "lucy_register_mqtt";

/// Parsed arguments for the `#[lucy_mqtt(...)]` attribute.
// Fields are currently consumed only to validate parsing; they will be used
// when the registration side-effect is emitted in a future task.
#[allow(dead_code)]
pub struct MqttArgs {
    /// MQTT topic string, e.g. `"sensors/temperature"`.
    pub topic: String,
    /// Optional human-readable description of the topic handler.
    pub description: Option<String>,
}

impl Parse for MqttArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Accumulators for each supported key; `Option` lets us detect missing
        // required keys and duplicate assignments.
        let mut topic: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;

        // Parse a comma-separated list of `key = "value"` pairs.
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            let _eq: Token![=] = input.parse()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "topic" => {
                    if topic.is_some() {
                        return Err(syn::Error::new_spanned(
                            &key,
                            "duplicate `topic` argument",
                        ));
                    }
                    topic = Some(value);
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
                            "unknown argument `{other}`; expected one of: topic, description"
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

        let topic = topic
            .ok_or_else(|| syn::Error::new(input.span(), "missing required `topic` argument"))?;

        Ok(MqttArgs {
            topic: topic.value(),
            description: description.map(|d| d.value()),
        })
    }
}

/// Expands the `#[lucy_mqtt(...)]` attribute.
///
/// Parses the attribute arguments, validates them, and returns the annotated
/// function unchanged. Registration code will be injected here in a future
/// task via [`REGISTRY_FN_IDENT`].
// TESTME: integration test in xtask
pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _args = parse_macro_input!(attr as MqttArgs);
    let func = parse_macro_input!(item as ItemFn);

    // Reference the constant so the compiler does not warn about it being
    // unused while the registration feature is still pending.
    let _ = REGISTRY_FN_IDENT;

    // TODO(registration): emit a side-effect call to REGISTRY_FN_IDENT
    // once lucy-core exposes a stable registration API.
    let expanded: TokenStream2 = quote! { #func };
    expanded.into()
}
