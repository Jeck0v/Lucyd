import { useState, useRef, useEffect, useId } from 'react'
import type { EndpointMeta, MqttMessage } from '../types'

// ---------------------------------------------------------------------------
// MqttTopicCard
// ---------------------------------------------------------------------------

interface MqttTopicCardProps {
  endpoint: EndpointMeta
  /** All messages captured by the shared useMqtt hook — filtered per topic inside. */
  messages: MqttMessage[]
  connected: boolean
  subscribe: (topic: string) => void
  unsubscribe: (topic: string) => void
  publish: (topic: string, payload: string) => void
}

/**
 * Interactive card for a single MQTT topic.
 *
 * Features:
 * - Collapsible header
 * - Subscribe / Unsubscribe toggle
 * - Payload input + Publish button
 * - Per-topic message log with timestamp
 */
export function MqttTopicCard({
  endpoint,
  messages,
  connected,
  subscribe,
  unsubscribe,
  publish,
}: MqttTopicCardProps): React.JSX.Element {
  const uid = useId()
  const topic = endpoint.path

  const [expanded, setExpanded] = useState(false)
  const [subscribed, setSubscribed] = useState(false)
  const [payload, setPayload] = useState('')

  // Filter messages that belong to this topic.
  const topicMessages = messages.filter((m) => m.topic === topic)

  const logEndRef = useRef<HTMLDivElement | null>(null)

  // Auto-scroll when new messages arrive.
  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [topicMessages.length])

  // If broker disconnects, reset subscribed state.
  useEffect(() => {
    if (!connected) {
      setSubscribed(false)
    }
  }, [connected])

  function handleSubscribeToggle(): void {
    if (subscribed) {
      unsubscribe(topic)
      setSubscribed(false)
    } else {
      subscribe(topic)
      setSubscribed(true)
    }
  }

  function handlePublish(): void {
    if (payload.trim() === '') return
    publish(topic, payload)
    setPayload('')
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>): void {
    if (e.key === 'Enter') handlePublish()
  }

  const headerId = `mqtt-card-header-${uid}`
  const bodyId = `mqtt-card-body-${uid}`

  return (
    <li className="endpoint-card">
      {/* ── Header ─────────────────────────────────────────────────────── */}
      <button
        id={headerId}
        className="endpoint-card__toggle"
        aria-expanded={expanded}
        aria-controls={bodyId}
        onClick={() => setExpanded((v) => !v)}
      >
        <span className="method-badge method-badge--mqtt">MQTT</span>
        <code className="endpoint-card__path">{topic}</code>
        {endpoint.description !== undefined && (
          <span className="endpoint-card__summary">{endpoint.description}</span>
        )}
        {subscribed && (
          <span className="status-dot status-dot--connected" aria-label="Subscribed" />
        )}
        <span className={`endpoint-card__chevron${expanded ? ' endpoint-card__chevron--open' : ''}`} aria-hidden="true">
          ›
        </span>
      </button>

      {/* ── Expandable body ────────────────────────────────────────────── */}
      {expanded && (
        <div id={bodyId} className="endpoint-card__body" role="region" aria-labelledby={headerId}>

          {/* Subscribe / Unsubscribe */}
          <div className="form-section form-section--inline">
            <button
              className={subscribed ? 'btn btn--danger' : 'btn btn--connect'}
              onClick={handleSubscribeToggle}
              disabled={!connected}
              aria-pressed={subscribed}
            >
              {subscribed ? 'Unsubscribe' : 'Subscribe'}
            </button>
            {!connected && (
              <span className="ws-status-error">Broker not connected</span>
            )}
          </div>

          {/* Publish */}
          <section className="form-section">
            <h3 className="form-section__title">
              <span className="form-section__badge">Publish</span>
            </h3>
            <div className="form-row">
              <input
                id={`${uid}-mqtt-payload`}
                className="form-input"
                type="text"
                placeholder="Payload"
                value={payload}
                onChange={(e) => setPayload(e.target.value)}
                onKeyDown={handleKeyDown}
                disabled={!connected}
                aria-label="Publish payload"
              />
              <button
                className="btn btn--primary"
                onClick={handlePublish}
                disabled={!connected || payload.trim() === ''}
              >
                Publish
              </button>
            </div>
          </section>

          {/* Message log */}
          <section className="form-section">
            <h3 className="form-section__title">
              <span className="form-section__badge">Received Messages</span>
            </h3>
            <div
              className="message-log"
              aria-live="polite"
              aria-label={`MQTT messages for topic ${topic}`}
            >
              {topicMessages.length === 0 ? (
                <p className="message-log__empty">No messages received yet.</p>
              ) : (
                topicMessages.map((msg) => (
                  <div key={msg.id} className="message-log__item message-log__item--in">
                    <span className="message-log__time">{msg.timestamp}</span>
                    <span className="message-log__direction" aria-hidden="true">←</span>
                    <span className="message-log__payload">{msg.payload}</span>
                  </div>
                ))
              )}
              <div ref={logEndRef} />
            </div>
          </section>
        </div>
      )}
    </li>
  )
}
