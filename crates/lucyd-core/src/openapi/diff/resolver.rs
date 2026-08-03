//! Follows `$ref` pointers back to the schema they name.
//!
//! Two documents being diffed almost never agree on component *names*. Lucyd
//! derives them from Rust type names (`{endpoint}_request`), a hand-written
//! spec from whatever its author chose. Comparing `$ref` strings would
//! therefore report a difference on every single operation. Resolving them to
//! the schemas they point at, and comparing those, is what makes the diff
//! meaningful.

use serde_json::Value;

/// Maximum `$ref` hops followed before a chain is declared cyclic.
///
/// Matches the bound used by the OpenAPI importer, so both directions of the
/// migration workflow give up on the same documents.
const MAX_REF_HOPS: usize = 32;

/// Resolves `$ref` pointers against the single document they belong to.
///
/// Constructing one is free (it borrows the document), so callers create them
/// on demand rather than threading one through every signature.
#[derive(Clone, Copy)]
pub(super) struct SchemaResolver<'a> {
    document: &'a Value,
}

impl<'a> SchemaResolver<'a> {
    /// Builds a resolver for `document`, the root of the OpenAPI document that
    /// every `$ref` in it is relative to.
    pub(super) fn new(document: &'a Value) -> Self {
        Self { document }
    }

    /// Follows `schema`'s `$ref` chain to the concrete schema it names,
    /// returning `schema` itself when it isn't a reference.
    ///
    /// Returns `None` for the three ways a reference can fail to resolve: it
    /// points outside this document, it points at nothing, or it loops.
    /// Callers surface that as a
    /// [`ChangeKind::Unresolved`](super::report::ChangeKind::Unresolved)
    /// rather than as "no difference".
    pub(super) fn resolve(&self, schema: &'a Value) -> Option<&'a Value> {
        let mut current = schema;
        for _ in 0..MAX_REF_HOPS {
            let Some(reference) = reference_of(current) else {
                return Some(current);
            };
            let pointer = reference.strip_prefix('#')?;
            current = self.document.pointer(pointer)?;
        }
        None
    }
}

/// Reads the `$ref` string off a schema, if it is a reference at all.
pub(super) fn reference_of(schema: &Value) -> Option<&str> {
    schema.get("$ref")?.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn document() -> Value {
        json!({
            "components": {
                "schemas": {
                    "User": { "type": "object" },
                    "Alias": { "$ref": "#/components/schemas/User" },
                    "Loop": { "$ref": "#/components/schemas/Loop" }
                }
            }
        })
    }

    /// Resolves `schema` against [`document`], returning an owned value so the
    /// borrow of the temporary document ends inside this helper.
    fn resolve(schema: Value) -> Option<Value> {
        let document = document();
        SchemaResolver::new(&document).resolve(&schema).cloned()
    }

    #[test]
    fn a_plain_schema_resolves_to_itself() {
        let schema = json!({ "type": "string" });
        assert_eq!(resolve(schema.clone()), Some(schema));
    }

    #[test]
    fn a_local_ref_resolves_to_its_target() {
        let resolved = resolve(json!({ "$ref": "#/components/schemas/User" }));
        assert_eq!(resolved, Some(json!({ "type": "object" })));
    }

    #[test]
    fn a_chain_of_refs_is_followed_to_the_end() {
        let resolved = resolve(json!({ "$ref": "#/components/schemas/Alias" }));
        assert_eq!(
            resolved,
            Some(json!({ "type": "object" })),
            "an alias component must resolve through to the schema it aliases"
        );
    }

    #[test]
    fn an_external_ref_does_not_resolve() {
        let resolved = resolve(json!({ "$ref": "other.yaml#/components/schemas/User" }));
        assert_eq!(resolved, None, "cross-document refs are out of scope");
    }

    #[test]
    fn a_dangling_ref_does_not_resolve() {
        let resolved = resolve(json!({ "$ref": "#/components/schemas/Missing" }));
        assert_eq!(resolved, None);
    }

    #[test]
    fn a_self_referential_ref_does_not_resolve() {
        let resolved = resolve(json!({ "$ref": "#/components/schemas/Loop" }));
        assert_eq!(resolved, None, "a ref cycle must terminate, not hang");
    }

    #[test]
    fn reference_of_reads_only_string_refs() {
        assert_eq!(reference_of(&json!({ "$ref": "#/x" })), Some("#/x"));
        assert_eq!(reference_of(&json!({ "type": "object" })), None);
        assert_eq!(reference_of(&json!({ "$ref": 42 })), None);
    }
}
