//! Maps JSON Schema objects (as found inside an OpenAPI document) to Rust
//! types, emitting `struct`/`enum` definitions as it goes.
//!
//! This is the inverse of `crates/lucy-core/src/openapi/components.rs`: that
//! module flattens schemars' draft-07 `definitions` into a de-duplicated
//! `components.schemas` map; [`TypeGenerator`] walks the same kind of shape
//! in the opposite direction, turning a `components.schemas` entry (or an
//! inline schema) into a Rust type, caching by name so a `$ref`'d component
//! referenced by several operations is only ever generated once.

use proc_macro2::Ident;
use quote::{format_ident, quote};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// A small set of Rust reserved keywords that could otherwise collide with a
/// sanitized field/variant/struct name derived from a JSON Schema property or
/// enum value. Not exhaustive (raw identifiers exist for a reason), but
/// covers the keywords realistically found in JSON API payloads (`type`,
/// `match`, `move`, `ref`, ...).
const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "async", "await", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
];

/// Generates Rust `struct`/`enum` items from JSON Schema objects, caching
/// already-generated component names for reuse.
pub struct TypeGenerator<'a> {
    /// The full OpenAPI document, needed to look up `#/components/schemas/X`
    /// targets when a schema uses `$ref`.
    document: &'a Value,
    /// Emission-ordered `struct`/`enum` items produced so far.
    items: Vec<syn::Item>,
    /// Every name already claimed by an emitted item (used to keep inline
    /// `{Parent}{Field}` names from colliding with each other or with
    /// `$ref`'d component names).
    names: HashSet<String>,
    /// Component name (as it appears after `#/components/schemas/`) to the
    /// identifier it was emitted under — the dedup cache proper.
    ref_cache: HashMap<String, Ident>,
    /// Component names currently being resolved, guarding against infinite
    /// recursion on a self-referential schema (an edge case genuinely out of
    /// scope for a `todo!()`-stub generator: representing it in Rust would
    /// need a `Box<T>` indirection this mapper doesn't attempt).
    in_progress: HashSet<String>,
}

impl<'a> TypeGenerator<'a> {
    /// Creates a generator that resolves `$ref`s against `document`.
    pub fn new(document: &'a Value) -> Self {
        Self {
            document,
            items: Vec::new(),
            names: HashSet::new(),
            ref_cache: HashMap::new(),
            in_progress: HashSet::new(),
        }
    }

    /// Consumes the generator, returning every `struct`/`enum` item emitted,
    /// in the order they were first generated.
    pub fn into_items(self) -> Vec<syn::Item> {
        self.items
    }

    /// Maps `schema` to a Rust type, emitting a new `struct`/`enum` under
    /// `hint` when the schema requires one (object-with-properties, or a
    /// string enum). `hint` is used verbatim (after sanitization) as the
    /// preferred name — callers are expected to have already computed it as
    /// a component name, `{Op}Request`/`{Op}Response`, or `{Parent}{Field}`.
    ///
    /// Errors when `schema`, or anything reachable from it (through
    /// `properties`, `items`, or `$ref`), uses `oneOf`/`allOf`/`anyOf`/`not`
    /// — none of these compose into a single Rust type.
    pub fn generate_type(&mut self, schema: &Value, hint: &str) -> Result<syn::Type, String> {
        reject_unsupported_composition(schema)?;

        if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
            return self.generate_ref(reference);
        }

        if schema.get("enum").is_some() {
            return self.generate_enum(schema, hint);
        }

        match schema.get("type").and_then(Value::as_str) {
            Some("object") if schema.get("properties").is_some() => {
                self.generate_struct(schema, hint)
            }
            Some("object") => {
                Ok(syn::parse_quote! { ::std::collections::HashMap<String, ::serde_json::Value> })
            }
            Some("array") => self.generate_array(schema, hint),
            Some("string") => Ok(syn::parse_quote! { String }),
            Some("integer") => {
                if schema.get("format").and_then(Value::as_str) == Some("int32") {
                    Ok(syn::parse_quote! { i32 })
                } else {
                    Ok(syn::parse_quote! { i64 })
                }
            }
            Some("number") => Ok(syn::parse_quote! { f64 }),
            Some("boolean") => Ok(syn::parse_quote! { bool }),
            // No `type` keyword at all but `properties` present: schemas
            // routinely omit the (redundant) `type: object` in the wild.
            None if schema.get("properties").is_some() => self.generate_struct(schema, hint),
            // Genuinely untyped (or a type we don't special-case): accept
            // any JSON value rather than guessing.
            _ => Ok(syn::parse_quote! { ::serde_json::Value }),
        }
    }

    /// Resolves a `#/components/schemas/{name}` reference, reusing the
    /// previously generated type if `{name}` was already emitted.
    fn generate_ref(&mut self, reference: &str) -> Result<syn::Type, String> {
        let name = reference
            .strip_prefix("#/components/schemas/")
            .ok_or_else(|| {
                format!(
                    "unsupported $ref (only #/components/schemas/... is supported): {reference}"
                )
            })?;

        if let Some(ident) = self.ref_cache.get(name) {
            return Ok(syn::parse_quote! { #ident });
        }
        if !self.in_progress.insert(name.to_string()) {
            return Err(format!(
                "circular $ref detected while resolving: {reference}"
            ));
        }

        let target = self
            .document
            .pointer(&format!("/components/schemas/{name}"))
            .cloned()
            .ok_or_else(|| format!("$ref target not found in document: {reference}"))?;

        let ty = self.generate_type(&target, name)?;
        self.in_progress.remove(name);

        if let syn::Type::Path(type_path) = &ty {
            if let Some(segment) = type_path.path.segments.last() {
                self.ref_cache
                    .insert(name.to_string(), segment.ident.clone());
            }
        }
        Ok(ty)
    }

    /// Emits a `struct` for an `object` schema with `properties`.
    fn generate_struct(&mut self, schema: &Value, hint: &str) -> Result<syn::Type, String> {
        let name = self.unique_name(hint);
        let ident = format_ident!("{name}");

        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let required: HashSet<&str> = schema
            .get("required")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();

        let mut fields = Vec::with_capacity(properties.len());
        for (field_name, field_schema) in &properties {
            let field_hint = format!("{name}{}", to_pascal_case(field_name));
            let inner_ty = self.generate_type(field_schema, &field_hint)?;
            let is_required = required.contains(field_name.as_str());

            let sanitized = sanitize_ident(&to_snake_case(field_name));
            let field_ident = format_ident!("{sanitized}");
            let rename = rename_attr_if_needed(field_name, &sanitized);

            let field_tokens = if is_required {
                quote! {
                    #rename
                    pub #field_ident: #inner_ty,
                }
            } else {
                quote! {
                    #rename
                    #[serde(skip_serializing_if = "Option::is_none")]
                    pub #field_ident: Option<#inner_ty>,
                }
            };
            fields.push(field_tokens);
        }

        let item_tokens = quote! {
            #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
            pub struct #ident {
                #(#fields)*
            }
        };
        self.register_item(&name, item_tokens);
        Ok(syn::parse_quote! { #ident })
    }

    /// Emits a fieldless `enum` for a `string` schema carrying an `enum` list.
    fn generate_enum(&mut self, schema: &Value, hint: &str) -> Result<syn::Type, String> {
        let name = self.unique_name(hint);
        let ident = format_ident!("{name}");

        let values = schema
            .get("enum")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut variants = Vec::with_capacity(values.len());
        for value in &values {
            let raw = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            let variant_name = sanitize_ident(&to_pascal_case(&raw));
            let variant_ident = format_ident!("{variant_name}");
            let rename = rename_attr_if_needed(&raw, &variant_name);
            variants.push(quote! {
                #rename
                #variant_ident,
            });
        }

        let item_tokens = quote! {
            #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
            pub enum #ident {
                #(#variants)*
            }
        };
        self.register_item(&name, item_tokens);
        Ok(syn::parse_quote! { #ident })
    }

    /// Maps an `array` schema's `items` to `Vec<T>`.
    fn generate_array(&mut self, schema: &Value, hint: &str) -> Result<syn::Type, String> {
        let items = schema.get("items").cloned().unwrap_or(Value::Null);
        let item_hint = format!("{hint}Item");
        let inner = self.generate_type(&items, &item_hint)?;
        Ok(syn::parse_quote! { Vec<#inner> })
    }

    /// Registers a freshly emitted item's name and pushes its tokens onto
    /// [`Self::items`], parsing them into a [`syn::Item`] so `codegen.rs`
    /// only ever has to work with `syn` types, never raw token streams.
    fn register_item(&mut self, name: &str, item_tokens: proc_macro2::TokenStream) {
        self.names.insert(name.to_string());
        let item: syn::Item =
            syn::parse2(item_tokens).expect("generated struct/enum tokens must parse as an Item");
        self.items.push(item);
    }

    /// Returns a name based on `base` that isn't already claimed by a
    /// previously emitted item, suffixing `_2`, `_3`, ... on collision.
    fn unique_name(&self, base: &str) -> String {
        let base = sanitize_ident(base);
        if !self.names.contains(&base) {
            return base;
        }
        let mut suffix = 2u32;
        loop {
            let candidate = format!("{base}_{suffix}");
            if !self.names.contains(&candidate) {
                return candidate;
            }
            suffix += 1;
        }
    }
}

/// Errors when `schema`'s own top level uses `oneOf`/`allOf`/`anyOf`/`not`.
/// [`TypeGenerator::generate_type`] recurses into every reachable subtree
/// (`properties`, `items`, resolved `$ref` targets) and re-applies this
/// check at each level, so "anywhere in the subtree" falls out of the normal
/// traversal rather than needing a separate deep scan here.
fn reject_unsupported_composition(schema: &Value) -> Result<(), String> {
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    for keyword in ["oneOf", "allOf", "anyOf", "not"] {
        if object.contains_key(keyword) {
            return Err(format!(
                "unsupported JSON Schema keyword `{keyword}`: it does not map to a single Rust type"
            ));
        }
    }
    Ok(())
}

/// Builds a `#[serde(rename = "...")]` attribute when the sanitized Rust
/// identifier differs from the original JSON name — otherwise no attribute
/// is needed since serde's default (de)serialization already matches.
fn rename_attr_if_needed(original: &str, rust_name: &str) -> proc_macro2::TokenStream {
    if original == rust_name {
        quote! {}
    } else {
        quote! { #[serde(rename = #original)] }
    }
}

/// Converts an arbitrary JSON Schema property/field name to `snake_case`,
/// sanitized into a valid Rust identifier fragment (callers still run the
/// result through [`sanitize_ident`] once more after any further mutation).
pub fn to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let mut prev_is_lower_or_digit = false;
    for c in input.chars() {
        if c == '-' || c == ' ' || c == '.' {
            result.push('_');
            prev_is_lower_or_digit = false;
            continue;
        }
        if c.is_uppercase() {
            if prev_is_lower_or_digit {
                result.push('_');
            }
            result.extend(c.to_lowercase());
            prev_is_lower_or_digit = false;
        } else {
            result.push(c);
            prev_is_lower_or_digit = c.is_lowercase() || c.is_ascii_digit();
        }
    }
    sanitize_ident(&result)
}

/// Converts an arbitrary name to `PascalCase`, used both for generated
/// struct/enum names derived from an operation id and for enum variant
/// names derived from an `enum` value.
pub fn to_pascal_case(input: &str) -> String {
    input
        .split(['_', '-', ' ', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Sanitizes an arbitrary string into a valid Rust identifier: strips
/// non-alphanumeric/underscore characters, guards against a leading digit,
/// falls back to a placeholder when empty, and dodges reserved keywords.
pub fn sanitize_ident(raw: &str) -> String {
    let mut sanitized: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if sanitized.is_empty() {
        sanitized = "field".to_string();
    }
    if sanitized.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        sanitized = format!("_{sanitized}");
    }
    if RUST_KEYWORDS.contains(&sanitized.as_str()) {
        sanitized.push('_');
    }
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_document() -> Value {
        json!({ "openapi": "3.1.0", "paths": {} })
    }

    /// Renders a `syn::Type` back to a string for readable assertions.
    fn type_string(ty: &syn::Type) -> String {
        quote! { #ty }.to_string()
    }

    #[test]
    fn object_with_properties_becomes_a_struct() {
        let document = empty_document();
        let mut generator = TypeGenerator::new(&document);
        let schema = json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        });

        let ty = generator
            .generate_type(&schema, "Widget")
            .expect("must succeed");
        assert_eq!(type_string(&ty), "Widget");

        let items = generator.into_items();
        assert_eq!(items.len(), 1);
        let rendered = quote! { #(#items)* }.to_string();
        assert!(rendered.contains("pub struct Widget"));
        assert!(rendered.contains("pub name : String"));
        assert!(rendered.contains("derive (Debug , Clone , Serialize , Deserialize , JsonSchema)"));
    }

    #[test]
    fn optional_field_becomes_option_with_skip_serializing() {
        let document = empty_document();
        let mut generator = TypeGenerator::new(&document);
        let schema = json!({
            "type": "object",
            "properties": { "nickname": { "type": "string" } }
        });

        generator
            .generate_type(&schema, "Widget")
            .expect("must succeed");
        let items = generator.into_items();
        let rendered = quote! { #(#items)* }.to_string();

        assert!(rendered.contains("pub nickname : Option < String >"));
        assert!(rendered.contains("skip_serializing_if = \"Option::is_none\""));
    }

    #[test]
    fn array_becomes_vec() {
        let document = empty_document();
        let mut generator = TypeGenerator::new(&document);
        let schema = json!({ "type": "array", "items": { "type": "string" } });

        let ty = generator
            .generate_type(&schema, "Tags")
            .expect("must succeed");
        assert_eq!(type_string(&ty), "Vec < String >");
    }

    #[test]
    fn string_enum_becomes_fieldless_enum() {
        let document = empty_document();
        let mut generator = TypeGenerator::new(&document);
        let schema = json!({ "type": "string", "enum": ["pending", "in-progress", "done"] });

        let ty = generator
            .generate_type(&schema, "Status")
            .expect("must succeed");
        assert_eq!(type_string(&ty), "Status");

        let items = generator.into_items();
        let rendered = quote! { #(#items)* }.to_string();
        assert!(rendered.contains("pub enum Status"));
        assert!(rendered.contains("Pending"));
        assert!(rendered.contains("Done"));
        // "in-progress" isn't a valid bare identifier fragment once
        // PascalCased across the hyphen, so it must carry an explicit rename.
        assert!(rendered.contains("rename = \"in-progress\""));
    }

    #[test]
    fn nested_object_becomes_parent_field_named_struct() {
        let document = empty_document();
        let mut generator = TypeGenerator::new(&document);
        let schema = json!({
            "type": "object",
            "properties": {
                "address": {
                    "type": "object",
                    "properties": { "city": { "type": "string" } },
                    "required": ["city"]
                }
            }
        });

        generator
            .generate_type(&schema, "User")
            .expect("must succeed");
        let items = generator.into_items();
        let rendered = quote! { #(#items)* }.to_string();

        assert!(rendered.contains("pub struct UserAddress"));
        assert!(rendered.contains("pub struct User"));
    }

    #[test]
    fn ref_to_same_component_is_generated_once_and_reused() {
        let document = json!({
            "openapi": "3.1.0",
            "paths": {},
            "components": {
                "schemas": {
                    "Address": {
                        "type": "object",
                        "properties": { "city": { "type": "string" } },
                        "required": ["city"]
                    }
                }
            }
        });
        let mut generator = TypeGenerator::new(&document);
        let reference = json!({ "$ref": "#/components/schemas/Address" });

        let first = generator
            .generate_type(&reference, "unused_hint_1")
            .expect("first ref resolution must succeed");
        let second = generator
            .generate_type(&reference, "unused_hint_2")
            .expect("second ref resolution must succeed");

        assert_eq!(type_string(&first), "Address");
        assert_eq!(type_string(&second), "Address");

        let items = generator.into_items();
        assert_eq!(
            items.len(),
            1,
            "the same $ref used twice must only emit one struct"
        );
    }

    #[test]
    fn one_of_is_rejected_with_an_error() {
        let document = empty_document();
        let mut generator = TypeGenerator::new(&document);
        let schema = json!({ "oneOf": [{ "type": "string" }, { "type": "integer" }] });

        let err = generator
            .generate_type(&schema, "Ambiguous")
            .err()
            .expect("oneOf must be rejected");
        assert!(err.contains("oneOf"));
    }

    #[test]
    fn nested_one_of_inside_properties_is_rejected() {
        let document = empty_document();
        let mut generator = TypeGenerator::new(&document);
        let schema = json!({
            "type": "object",
            "properties": {
                "value": { "oneOf": [{ "type": "string" }, { "type": "integer" }] }
            }
        });

        let err = generator
            .generate_type(&schema, "Wrapper")
            .err()
            .expect("nested oneOf must be rejected too");
        assert!(err.contains("oneOf"));
    }
}
