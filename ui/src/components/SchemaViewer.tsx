import { useState, useMemo } from 'react'
import { schemaExampleJson } from '../utils/schemaToExample'

interface SchemaViewerProps {
  schema: Record<string, unknown>
  /** Displayed label prefix, e.g. "Request" or "Response". */
  label: string
}

/**
 * Renders a JSON Schema in two selectable modes:
 * - "Example Value" — a generated representative value derived from the schema
 * - "Schema" — the raw JSON Schema object
 *
 * Tabs follow the WAI-ARIA `tablist` / `tab` pattern with `aria-selected`.
 */
export function SchemaViewer({ schema, label }: SchemaViewerProps): React.JSX.Element {
  const [mode, setMode] = useState<'example' | 'schema'>('example')

  const exampleJson = useMemo(() => schemaExampleJson(schema), [schema])
  const schemaJson = useMemo(() => JSON.stringify(schema, null, 2), [schema])

  const title = schema.title as string | undefined

  return (
    <div className="schema-viewer">
      <div className="schema-viewer__header">
        <span className="schema-viewer__label">
          {label}{title !== undefined ? `: ${title}` : ''}
        </span>
        <div className="schema-viewer__tabs" role="tablist">
          <button
            role="tab"
            aria-selected={mode === 'example'}
            className={`schema-tab${mode === 'example' ? ' schema-tab--active' : ''}`}
            onClick={() => setMode('example')}
          >
            Example Value
          </button>
          <button
            role="tab"
            aria-selected={mode === 'schema'}
            className={`schema-tab${mode === 'schema' ? ' schema-tab--active' : ''}`}
            onClick={() => setMode('schema')}
          >
            Schema
          </button>
        </div>
      </div>
      <pre className="schema-viewer__code">
        <code>{mode === 'example' ? exampleJson : schemaJson}</code>
      </pre>
    </div>
  )
}
