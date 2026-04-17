/**
 * Converts a JSON Schema (draft-07, as produced by `schemars`) into a
 * representative example value.
 *
 * The conversion is intentionally shallow — it resolves `$ref`, picks the
 * first branch of `oneOf`/`anyOf`, merges `allOf`, and generates typed
 * primitives. Depth is capped at 8 to prevent infinite recursion on
 * self-referential schemas.
 */

/** JSON Schema draft-07: a schema is either a boolean or an object. */
type JsonSchema = Record<string, unknown> | boolean

export function schemaToExample(
  schema: JsonSchema,
  definitions?: Record<string, JsonSchema>,
  depth = 0,
): unknown {
  if (depth > 8) return null

  // JSON Schema draft-07 allows boolean schemas:
  //   true  = any value is valid → use null as a placeholder
  //   false = no value is valid  → return null
  if (typeof schema === 'boolean') return schema ? null : null

  // Resolve $ref
  if ('$ref' in schema && typeof schema.$ref === 'string') {
    const refPath = schema.$ref as string
    if (refPath.startsWith('#/definitions/')) {
      const defName = refPath.replace('#/definitions/', '')
      const def = definitions?.[defName]
      if (def) return schemaToExample(def, definitions, depth + 1)
    }
    return null
  }

  // oneOf / anyOf → first option
  const oneOf = schema.oneOf ?? schema.anyOf
  if (Array.isArray(oneOf) && oneOf.length > 0) {
    return schemaToExample(oneOf[0] as JsonSchema, definitions, depth + 1)
  }

  // allOf → merge (simplified)
  if (Array.isArray(schema.allOf)) {
    const merged: Record<string, unknown> = {}
    for (const sub of schema.allOf as JsonSchema[]) {
      const ex = schemaToExample(sub, definitions, depth + 1)
      if (ex !== null && typeof ex === 'object' && !Array.isArray(ex)) {
        Object.assign(merged, ex)
      }
    }
    return merged
  }

  const type = schema.type as string | undefined

  // object
  if (type === 'object' || (!type && 'properties' in schema)) {
    const props = schema.properties as Record<string, JsonSchema> | undefined
    if (!props) return {}
    const result: Record<string, unknown> = {}
    for (const [key, propSchema] of Object.entries(props)) {
      result[key] = schemaToExample(propSchema, definitions, depth + 1)
    }
    return result
  }

  // array
  if (type === 'array') {
    const items = schema.items as JsonSchema | undefined
    if (!items) return []
    return [schemaToExample(items, definitions, depth + 1)]
  }

  // enum → first value
  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    return schema.enum[0]
  }

  // keyword shortcuts
  if ('default' in schema) return schema.default
  if ('example' in schema) return schema.example
  if ('const' in schema) return schema.const

  switch (type) {
    case 'string':
      return ''
    case 'integer':
    case 'number':
      return 0
    case 'boolean':
      return false
    case 'null':
      return null
    default:
      return null
  }
}

/**
 * Serialises a JSON Schema to a pretty-printed JSON example string.
 * Resolves top-level `definitions` automatically.
 */
export function schemaExampleJson(schema: JsonSchema): string {
  const defs =
    typeof schema === 'object' && schema !== null
      ? (schema.definitions as Record<string, JsonSchema> | undefined)
      : undefined
  const example = schemaToExample(schema, defs)
  return JSON.stringify(example, null, 2)
}
