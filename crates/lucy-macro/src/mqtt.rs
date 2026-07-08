//! Implementation of the `#[lucy_mqtt(...)]` attribute macro.
//!
//! Parses the attribute arguments into [`MqttArgs`] and emits the annotated
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

/// Parsed arguments for the `#[lucy_mqtt(...)]` attribute.
pub struct MqttArgs {
    /// MQTT topic string, e.g. `"sensors/temperature"`.
    pub topic: String,
    /// Optional human-readable description of the topic handler.
    pub description: Option<String>,
    /// Optional comma-separated tags for grouping in the documentation UI.
    pub tags: Vec<String>,
    /// Optional request/publish payload type for JSON Schema generation.
    pub request_type: Option<syn::Type>,
    /// Optional response/subscribe payload type for JSON Schema generation.
    pub response_type: Option<syn::Type>,
}

/// Accumulator for `#[lucy_mqtt(...)]` arguments as they're parsed.
///
/// Fields are `Option` so [`Parse::parse`] can detect both missing required
/// keys and duplicate assignments before handing off to [`RawMqttArgs::finalize`].
#[derive(Default)]
struct RawMqttArgs {
    topic: Option<LitStr>,
    description: Option<LitStr>,
    tags: Option<LitStr>,
    request_type: Option<syn::Type>,
    response_type: Option<syn::Type>,
}

impl RawMqttArgs {
    /// Validates required keys and converts raw tokens into [`MqttArgs`].
    fn finalize(self, span: Span) -> syn::Result<MqttArgs> {
        let topic = common::require(self.topic, "topic", span)?;

        Ok(MqttArgs {
            topic: topic.value(),
            description: self.description.map(|d| d.value()),
            tags: common::parse_tags(self.tags),
            request_type: self.request_type,
            response_type: self.response_type,
        })
    }
}

impl Parse for MqttArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut raw = RawMqttArgs::default();

        // Parse a comma-separated list of `key = value` pairs.
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "topic" => common::parse_unique(&key, &mut raw.topic, input)?,
                "description" => common::parse_unique(&key, &mut raw.description, input)?,
                "tags" => common::parse_unique(&key, &mut raw.tags, input)?,
                "request" => common::parse_unique(&key, &mut raw.request_type, input)?,
                "response" => common::parse_unique(&key, &mut raw.response_type, input)?,
                _ => {
                    return Err(common::unknown_argument_error(
                        &key,
                        "topic, description, tags, request, response",
                    ));
                }
            }

            common::consume_trailing_comma(input)?;
        }

        raw.finalize(input.span())
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
    let topic = &args.topic; // stored in the `path` field

    let description_tokens = common::option_str_tokens(args.description.as_deref());
    let tags_tokens = common::tags_tokens(&args.tags);
    let request_schema_tokens = common::schema_fn_tokens(args.request_type.as_ref());
    let response_schema_tokens = common::schema_fn_tokens(args.response_type.as_ref());

    let expanded = quote! {
        #func

        ::lucyd::_private::inventory::submit! {
            ::lucyd::_private::lucy_types::endpoint::EndpointMetaStatic {
                name:              #fn_name,
                path:              #topic,
                protocol:          ::lucyd::_private::lucy_types::endpoint::Protocol::Mqtt,
                description:       #description_tokens,
                method:            ::core::option::Option::None,
                tags:              #tags_tokens,
                request_schema_fn:  #request_schema_tokens,
                response_schema_fn: #response_schema_tokens,
            }
        }
    };
    expanded.into()
}
