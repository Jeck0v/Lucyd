/**
 * Reads the query parameters an endpoint declares and turns typed input values
 * back into a query string.
 *
 * The declaration comes from `EndpointMeta.query_schema`, the JSON Schema the
 * `query = T` macro argument produced. Shared by the HTTP and WebSocket cards:
 * both render the same inputs, and both have to append the same query string —
 * one to a `fetch` URL, the other to a `ws://` upgrade URL.
 */

/** A JSON object, as far as an untrusted `unknown` can be narrowed to one. */
type JsonObject = Record<string, unknown>

/** One declared query parameter, ready to render as a table row. */
export interface QueryParam {
  name: string
  /** JSON Schema type, e.g. `'integer'`; `'string'` when none is declared. */
  type: string
  /** Listed in the schema's `required` array — i.e. not an `Option<T>`. */
  required: boolean
  /** The Rust field's doc comment, or `undefined` when it had none. */
  description: string | undefined
}

/** Narrows `value` to a plain JSON object, or `undefined` if it is anything else. */
function asObject(value: unknown): JsonObject | undefined {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as JsonObject)
    : undefined
}

/**
 * The type to display for a property.
 *
 * Collapses schemars' `Option<T>` spelling, `['integer', 'null']`, down to the
 * concrete type: optionality is already conveyed by `required`, so showing
 * `integer | null` next to an input would say the same thing twice.
 */
function displayType(property: unknown): string {
  const type = asObject(property)?.type
  if (typeof type === 'string') return type

  const concrete = Array.isArray(type)
    ? type.filter((entry): entry is string => typeof entry === 'string' && entry !== 'null')
    : []
  return concrete.length === 1 ? concrete[0]! : 'string'
}

/**
 * Extracts one [`QueryParam`] per property of an endpoint's query schema, in
 * schema order. Returns an empty array for an endpoint that declares no
 * `query = T`, which is every endpoint written before the argument existed.
 */
export function extractQueryParams(schema: unknown): QueryParam[] {
  const properties = asObject(asObject(schema)?.properties)
  if (properties === undefined) return []

  const declaredRequired = asObject(schema)?.required
  const required = new Set(Array.isArray(declaredRequired) ? declaredRequired : [])

  return Object.entries(properties).map(([name, property]) => ({
    name,
    type: displayType(property),
    required: required.has(name),
    description: asObject(property)?.description as string | undefined,
  }))
}

/**
 * Appends `values` to `path` as a query string, skipping the empty ones.
 *
 * `URLSearchParams` does the percent-encoding, and the separator is chosen from
 * what `path` already carries so this composes with a path that is already
 * carrying a query string.
 */
export function appendQueryString(path: string, values: Array<[string, string]>): string {
  const query = new URLSearchParams(values.filter(([, value]) => value.trim() !== ''))
  const encoded = query.toString()
  if (encoded === '') return path

  return `${path}${path.includes('?') ? '&' : '?'}${encoded}`
}
