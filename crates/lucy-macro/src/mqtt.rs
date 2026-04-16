//! Implementation of the `#[lucy_mqtt(...)]` attribute macro.
//!
//! Parses the attribute arguments into [`MqttArgs`] and emits the annotated
//! function along with an `inventory::submit!` block that registers the
//! endpoint in the global [`EndpointRegistry`] at link time.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Ident, ItemFn, LitStr, Token,
};

/// Parsed arguments for the `#[lucy_mqtt(...)]` attribute.
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
/// Parses the attribute arguments, validates them, and emits the original
/// function together with an `inventory::submit!` block that registers the
/// endpoint metadata at link time.
pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as MqttArgs);
    let func = parse_macro_input!(item as ItemFn);

    let fn_name = func.sig.ident.to_string();
    let topic   = &args.topic;   // stored in the `path` field

    let description_tokens = match &args.description {
        Some(desc) => quote! { ::core::option::Option::Some(#desc) },
        None => quote! { ::core::option::Option::None },
    };

    let expanded = quote! {
        #func

        ::lucy::_private::inventory::submit! {
            ::lucy::_private::lucy_types::endpoint::EndpointMetaStatic {
                name:        #fn_name,
                path:        #topic,
                protocol:    ::lucy::_private::lucy_types::endpoint::Protocol::Mqtt,
                description: #description_tokens,
                method:      ::core::option::Option::None,
            }
        }
    };
    expanded.into()
}
