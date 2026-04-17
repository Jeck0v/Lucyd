import { useState, useRef, useEffect, useId } from 'react'
import type { EndpointMeta } from '../types'
import { useWebSocket } from '../hooks/useWebSocket'
import { useAuth } from '../context/AuthContext'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function extractPathParams(path: string): string[] {
  return [...path.matchAll(/\{([^}]+)\}/g)].map((m) => m[1] ?? '')
}

function resolvePath(path: string, params: Record<string, string>): string {
  return path.replace(/\{([^}]+)\}/g, (_, k: string) =>
    encodeURIComponent(params[k] ?? ''),
  )
}

function buildWsUrl(resolvedPath: string, bearerToken?: string): string {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const base = `${proto}//${location.host}${resolvedPath}`
  if (bearerToken && bearerToken.trim() !== '') {
    const sep = resolvedPath.includes('?') ? '&' : '?'
    return `${base}${sep}token=${encodeURIComponent(bearerToken.trim())}`
  }
  return base
}

// ---------------------------------------------------------------------------
// WsEndpointCard
// ---------------------------------------------------------------------------

interface WsEndpointCardProps {
  endpoint: EndpointMeta
}

/**
 * Interactive card for a single WebSocket endpoint.
 *
 * Features:
 * - Collapsible header with connection status dot + Connect/Disconnect button
 * - Path parameter inputs (auto-detected)
 * - Message input with Send button (disabled when not connected)
 * - Scrollable message log: ← incoming (cyan), → outgoing (grey-blue), timestamp
 * - Clear button to reset the log
 */
export function WsEndpointCard({ endpoint }: WsEndpointCardProps): React.JSX.Element {
  const uid = useId()
  const pathParams = extractPathParams(endpoint.path)
  const { auth } = useAuth()

  const [expanded, setExpanded] = useState(false)
  const [paramValues, setParamValues] = useState<Record<string, string>>(
    Object.fromEntries(pathParams.map((p) => [p, ''])),
  )
  const [messageInput, setMessageInput] = useState('')

  const { status, messages, lastClose, connect, disconnect, send, clearMessages } = useWebSocket()

  const logEndRef = useRef<HTMLDivElement | null>(null)

  // Auto-scroll to bottom whenever new messages arrive.
  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  function handleParamChange(param: string, value: string): void {
    setParamValues((prev) => ({ ...prev, [param]: value }))
  }

  function handleConnect(): void {
    const resolvedPath = resolvePath(endpoint.path, paramValues)
    // Browser WebSocket APIs do not support custom headers. When a bearer
    // token is configured, it is forwarded via the `?token=` query parameter
    // instead (the server must read it from there).
    const bearerToken = auth.type === 'bearer' ? auth.bearer : undefined
    const url = buildWsUrl(resolvedPath, bearerToken)
    connect(url)
  }

  function handleSend(): void {
    if (messageInput.trim() === '') return
    send(messageInput)
    setMessageInput('')
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>): void {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) handleSend()
  }

  const isConnected = status === 'connected'
  const isConnecting = status === 'connecting'

  const headerId = `ws-card-header-${uid}`
  const bodyId = `ws-card-body-${uid}`

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
        <span className="method-badge method-badge--ws">WS</span>
        <code className="endpoint-card__path">{endpoint.path}</code>
        {endpoint.description !== undefined && (
          <span className="endpoint-card__summary">{endpoint.description}</span>
        )}
        {/* Status dot — always visible in header */}
        <span
          className={`status-dot status-dot--${isConnected ? 'connected' : 'disconnected'}`}
          aria-label={isConnected ? 'Connected' : 'Disconnected'}
        />
        <span className={`endpoint-card__chevron${expanded ? ' endpoint-card__chevron--open' : ''}`} aria-hidden="true">
          ›
        </span>
      </button>

      {/* ── Expandable body ────────────────────────────────────────────── */}
      {expanded && (
        <div id={bodyId} className="endpoint-card__body" role="region" aria-labelledby={headerId}>

          {/* Path parameters */}
          {pathParams.length > 0 && (
            <section className="form-section">
              <h3 className="form-section__title">
                <span className="form-section__badge">Path Parameters</span>
              </h3>
              {pathParams.map((param) => (
                <div key={param} className="form-group">
                  <label className="form-label" htmlFor={`${uid}-ws-param-${param}`}>
                    {param}
                  </label>
                  <input
                    id={`${uid}-ws-param-${param}`}
                    className="form-input"
                    type="text"
                    placeholder={param}
                    value={paramValues[param] ?? ''}
                    onChange={(e) => handleParamChange(param, e.target.value)}
                    disabled={isConnected || isConnecting}
                  />
                </div>
              ))}
            </section>
          )}

          {/* Auth info */}
          {auth.type === 'bearer' && (
            <section className="form-section">
              <h3 className="form-section__title">
                <span className="form-section__badge">Authorization</span>
              </h3>
              <p className="auth-info-label">
                Bearer token will be appended as <code>?token=</code> query parameter.
              </p>
            </section>
          )}

          {/* Connection controls */}
          <div className="form-section form-section--inline">
            {!isConnected && status !== 'connecting' ? (
              <button className="btn btn--connect" onClick={handleConnect}>
                {status === 'error' ? 'Retry' : 'Connect'}
              </button>
            ) : isConnecting ? (
              <button className="btn btn--connect" disabled aria-busy>
                Connecting…
              </button>
            ) : (
              <button className="btn btn--danger" onClick={disconnect}>
                Disconnect
              </button>
            )}
          </div>

          {/* Error / close reason — persists until next connect attempt */}
          {status === 'error' && lastClose !== null && (
            <div className="response-panel response-panel--error" role="alert">
              <strong>Close {lastClose.code}</strong> — {lastClose.reason}
            </div>
          )}

          {/* Message send */}
          <section className="form-section">
            <h3 className="form-section__title">
              <span className="form-section__badge">Message Body</span>
            </h3>
            <textarea
              className="form-textarea"
              rows={5}
              placeholder={'{\n  "type": "ping"\n}'}
              value={messageInput}
              onChange={(e) => setMessageInput(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={!isConnected}
              spellCheck={false}
              aria-label="Message payload"
            />
            <div className="form-section form-section--actions">
              <button
                className="btn btn--primary"
                onClick={handleSend}
                disabled={!isConnected || messageInput.trim() === ''}
              >
                Send
              </button>
            </div>
          </section>

          {/* Message log */}
          <section className="form-section">
            <div className="form-section__row">
              <h3 className="form-section__title">
                <span className="form-section__badge">Message Log</span>
              </h3>
              <button
                className="btn btn--ghost"
                onClick={clearMessages}
                disabled={messages.length === 0}
              >
                Clear
              </button>
            </div>
            <div className="message-log" aria-live="polite" aria-label="WebSocket message log">
              {messages.length === 0 ? (
                <p className="message-log__empty">No messages yet.</p>
              ) : (
                messages.map((msg) => (
                  <div
                    key={msg.id}
                    className={`message-log__item message-log__item--${msg.direction}`}
                  >
                    <span className="message-log__time">{msg.timestamp}</span>
                    <span className="message-log__direction" aria-hidden="true">
                      {msg.direction === 'in' ? '←' : '→'}
                    </span>
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
