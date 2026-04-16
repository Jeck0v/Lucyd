import { useState, useId } from 'react'
import type { EndpointMeta } from '../types'
import { useMqtt } from '../hooks/useMqtt'
import { MqttTopicCard } from '../components/MqttTopicCard'
import { groupByTag } from '../utils/groupByTag'

/** Props accepted by the MqttPanel component. */
interface MqttPanelProps {
  endpoints: EndpointMeta[]
}

// ---------------------------------------------------------------------------
// EndpointGroup — collapsible section for a single tag
// ---------------------------------------------------------------------------

interface EndpointGroupProps {
  tag: string
  endpoints: EndpointMeta[]
  messages: ReturnType<typeof useMqtt>['messages']
  connected: boolean
  subscribe: (topic: string) => void
  unsubscribe: (topic: string) => void
  publish: (topic: string, payload: string) => void
}

function EndpointGroup({
  tag,
  endpoints,
  messages,
  connected,
  subscribe,
  unsubscribe,
  publish,
}: EndpointGroupProps): React.JSX.Element {
  const [open, setOpen] = useState(true)

  return (
    <section className="endpoint-group">
      <button
        className={[
          'endpoint-group__header',
          open ? 'endpoint-group__header--open' : '',
        ]
          .filter(Boolean)
          .join(' ')}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="endpoint-group__tag">{tag}</span>
        <span className="endpoint-group__count">{endpoints.length}</span>
        <span className="endpoint-group__chevron" aria-hidden="true">▼</span>
      </button>

      {open && (
        <div className="endpoint-group__body">
          <ul className="endpoint-list">
            {endpoints.map((endpoint) => (
              <MqttTopicCard
                key={endpoint.path}
                endpoint={endpoint}
                messages={messages}
                connected={connected}
                subscribe={subscribe}
                unsubscribe={unsubscribe}
                publish={publish}
              />
            ))}
          </ul>
        </div>
      )}
    </section>
  )
}

// ---------------------------------------------------------------------------
// MqttPanel
// ---------------------------------------------------------------------------

/**
 * MQTT panel — manages a single broker connection shared across all topic cards.
 *
 * Layout:
 * 1. Broker bar at the top: URL input + Connect / Disconnect + status dot
 * 2. Topic cards grouped by tag, each group collapsible independently.
 *
 * All topic cards share the same `useMqtt` instance so that subscribe /
 * publish / message callbacks are routed through a single connection.
 */
export function MqttPanel({ endpoints }: MqttPanelProps): React.JSX.Element {
  const uid = useId()
  const [brokerInput, setBrokerInput] = useState('ws://localhost:9001')
  // The active broker URL passed to useMqtt. Empty string = disconnected.
  const [activeBrokerUrl, setActiveBrokerUrl] = useState('')

  const { connected, messages, subscribe, unsubscribe, publish } =
    useMqtt(activeBrokerUrl)

  function handleConnect(): void {
    setActiveBrokerUrl(brokerInput.trim())
  }

  function handleDisconnect(): void {
    // Setting the URL to empty string causes useMqtt to call client.end().
    setActiveBrokerUrl('')
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>): void {
    if (e.key === 'Enter' && !connected) handleConnect()
  }

  const groups = groupByTag(endpoints)

  return (
    <div>
      {/* ── Broker connection bar ───────────────────────────────────────── */}
      <div className="broker-bar">
        <span
          className={`status-dot ${connected ? 'status-dot--connected' : 'status-dot--disconnected'}`}
          aria-label={connected ? 'Broker connected' : 'Broker disconnected'}
        />
        <label className="form-label broker-bar__label" htmlFor={`${uid}-broker-url`}>
          Broker
        </label>
        <input
          id={`${uid}-broker-url`}
          className="form-input broker-bar__input"
          type="url"
          placeholder="ws://localhost:9001"
          value={brokerInput}
          onChange={(e) => setBrokerInput(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={connected}
          aria-label="MQTT broker WebSocket URL"
        />
        {!connected ? (
          <button
            className="btn btn--connect"
            onClick={handleConnect}
            disabled={brokerInput.trim() === ''}
          >
            Connect
          </button>
        ) : (
          <button className="btn btn--danger" onClick={handleDisconnect}>
            Disconnect
          </button>
        )}
      </div>

      {/* ── Topic cards grouped by tag ──────────────────────────────────── */}
      {endpoints.length === 0 ? (
        <p className="panel-empty">No MQTT topics defined in the spec.</p>
      ) : (
        [...groups.entries()].map(([tag, tagEndpoints]) => (
          <EndpointGroup
            key={tag}
            tag={tag}
            endpoints={tagEndpoints}
            messages={messages}
            connected={connected}
            subscribe={subscribe}
            unsubscribe={unsubscribe}
            publish={publish}
          />
        ))
      )}
    </div>
  )
}
