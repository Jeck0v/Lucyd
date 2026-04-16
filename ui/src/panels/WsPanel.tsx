import type { EndpointMeta } from '../types'

/** Props accepted by the WsPanel component. */
interface WsPanelProps {
  endpoints: EndpointMeta[]
}

/**
 * Renders a list of WebSocket endpoint cards.
 *
 * Each card shows the route path and an optional description. This is a
 * skeleton component — live connection management will be implemented in
 * a subsequent iteration.
 */
export function WsPanel({ endpoints }: WsPanelProps): React.JSX.Element {
  if (endpoints.length === 0) {
    return (
      <p className="panel-empty">No WebSocket endpoints defined in the spec.</p>
    )
  }

  return (
    <ul className="endpoint-list">
      {endpoints.map((endpoint) => (
        <li key={endpoint.path} className="endpoint-card">
          <div className="endpoint-card__header">
            <span className="method-badge method-badge--ws">WS</span>
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
