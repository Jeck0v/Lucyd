import { useMemo } from 'react'
import type { EndpointMeta } from '../types'
import { SchemaViewer } from '../components/SchemaViewer'

interface ModelsPanelProps {
  endpoints: EndpointMeta[]
}

/** The schema-carrying fields of an endpoint, in the order they are listed. */
const SCHEMA_FIELDS = ['query_schema', 'request_schema', 'response_schema'] as const

/**
 * Collects every unique JSON Schema an endpoint declares — `query_schema`,
 * `request_schema` and `response_schema` — across all endpoints, deduplicates
 * by `title` (or a generated key), and renders each with a `SchemaViewer`.
 */
export function ModelsPanel({ endpoints }: ModelsPanelProps): React.JSX.Element {
  const models = useMemo(() => {
    const seen = new Map<string, Record<string, unknown>>()

    for (const ep of endpoints) {
      for (const field of SCHEMA_FIELDS) {
        const schema = ep[field]
        if (!schema) continue
        const key =
          (schema.title as string | undefined) ??
          `${ep.name}_${field.replace('_schema', '')}`
        if (!seen.has(key)) seen.set(key, schema)
      }
    }

    return [...seen.entries()]
  }, [endpoints])

  if (models.length === 0) {
    return (
      <p className="panel-empty">
        No schemas available yet. Annotate your handlers with{' '}
        <code>request = MyType</code> or <code>response = MyType</code>.
      </p>
    )
  }

  return (
    <div className="models-panel">
      {models.map(([key, schema]) => (
        <div key={key} className="models-panel__item">
          <SchemaViewer schema={schema} label={key} />
        </div>
      ))}
    </div>
  )
}
