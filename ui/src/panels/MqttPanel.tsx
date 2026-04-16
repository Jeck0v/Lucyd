import type { EndpointMeta } from '../types'

/** Props accepted by the MqttPanel component. */
interface MqttPanelProps {
  endpoints: EndpointMeta[]
}

/**
 * Renders a list of MQTT topic cards.
 *
 * Each card shows the topic path and an optional description. This is a
 * skeleton component — broker connection and pub/sub will be wired up
 * in a subsequent iteration using the `useMqtt` hook.
 */
export function MqttPanel({ endpoints }: MqttPanelProps): React.JSX.Element {
  if (endpoints.length === 0) {
    return (
      <p className="panel-empty">No MQTT topics defined in the spec.</p>
    )
  }

  return (
    <ul className="endpoint-list">
      {endpoints.map((endpoint) => (
        <li key={endpoint.path} className="endpoint-card">
          <div className="endpoint-card__header">
            <span className="method-badge method-badge--mqtt">MQTT</span>
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
