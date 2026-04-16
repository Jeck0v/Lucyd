import type { EndpointMeta } from '../types'

/** Props accepted by the HttpPanel component. */
interface HttpPanelProps {
  endpoints: EndpointMeta[]
}

/**
 * Renders a list of HTTP endpoint cards.
 *
 * Each card shows the HTTP method badge, the route path, and an optional
 * human-readable description. This is a skeleton component — request sending
 * will be implemented in a subsequent iteration.
 */
export function HttpPanel({ endpoints }: HttpPanelProps): React.JSX.Element {
  if (endpoints.length === 0) {
    return (
      <p className="panel-empty">No HTTP endpoints defined in the spec.</p>
    )
  }

  return (
    <ul className="endpoint-list">
      {endpoints.map((endpoint) => (
        <li key={`${endpoint.method ?? 'GET'}-${endpoint.path}`} className="endpoint-card">
          <div className="endpoint-card__header">
            <span className={`method-badge method-badge--${(endpoint.method ?? 'GET').toLowerCase()}`}>
              {endpoint.method ?? 'GET'}
            </span>
            <code className="endpoint-card__path">{endpoint.path}</code>
          </div>
          {endpoint.description !== undefined && (
            <p className="endpoint-card__description">{endpoint.description}</p>
          )}
        </li>
      ))}
    </ul>
  )
}
