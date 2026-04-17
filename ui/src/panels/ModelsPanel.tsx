import { useMemo } from 'react'
import type { EndpointMeta } from '../types'
import { SchemaViewer } from '../components/SchemaViewer'

interface ModelsPanelProps {
  endpoints: EndpointMeta[]
}

/**
 * Collects every unique JSON Schema from `request_schema` and `response_schema`
 * across all endpoints, deduplicates by `title` (or a generated key), and
 * renders each with a `SchemaViewer`.
 */
export function ModelsPanel({ endpoints }: ModelsPanelProps): React.JSX.Element {
  const models = useMemo(() => {
    const seen = new Map<string, Record<string, unknown>>()

    for (const ep of endpoints) {
      if (ep.request_schema) {
        const s = ep.request_schema as Record<string, unknown>
        const key = (s.title as string | undefined) ?? `${ep.name}_request`
        if (!seen.has(key)) seen.set(key, s)
      }
      if (ep.response_schema) {
        const s = ep.response_schema as Record<string, unknown>
        const key = (s.title as string | undefined) ?? `${ep.name}_response`
        if (!seen.has(key)) seen.set(key, s)
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
