import { useState, useId } from 'react'
import type { EndpointMeta } from '../types'
import { useAuth, buildAuthHeaders } from '../context/AuthContext'
import { SchemaViewer } from './SchemaViewer'
import { schemaExampleJson } from '../utils/schemaToExample'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** HTTP methods that carry a request body. */
const BODY_METHODS = new Set(['POST', 'PUT', 'PATCH', 'DELETE'])

/** Extracts the names of all `{param}` placeholders from a path template. */
function extractPathParams(path: string): string[] {
  return [...path.matchAll(/\{([^}]+)\}/g)].map((m) => m[1] ?? '')
}

/** Replaces `{param}` placeholders in a path with URL-encoded values. */
function resolvePath(path: string, params: Record<string, string>): string {
  return path.replace(/\{([^}]+)\}/g, (_, k: string) =>
    encodeURIComponent(params[k] ?? ''),
  )
}

/** Pretty-prints a JSON string, falling back to the raw string on failure. */
function prettyPrint(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw) as unknown, null, 2)
  } catch {
    return raw
  }
}

/** Maps an HTTP status code to a CSS modifier for `.status-badge`. */
function statusBadgeModifier(status: number): string {
  if (status >= 200 && status < 300) return 'ok'
  if (status >= 300 && status < 500) return 'warn'
  return 'error'
}

/**
 * Generates a placeholder JSON body for endpoints that accept a request body.
 *
 * When `request_schema` becomes available in the spec, this function can be
 * extended to produce a fully-typed skeleton. For now it emits a note object
 * so the textarea is never empty on open.
 */
function generateExampleBody(endpoint: EndpointMeta): string {
  const parts = endpoint.path.split('/').filter(Boolean)
  const resource = parts[parts.length - 1] ?? 'resource'

  return JSON.stringify(
    {
      _note: `Example body for ${endpoint.name} — update with actual fields`,
      // Surface the resource name as a hint for the developer.
      ...(resource && !resource.startsWith('{')
        ? { [resource]: '' }
        : {}),
    },
    null,
    2,
  )
}

/**
 * Builds a cURL command string equivalent to the given request.
 *
 * The URL is prefixed with `window.location.origin` so it targets the same
 * host as the Lucy UI, which is co-located with the Axum backend.
 */
function buildCurl(
  method: string,
  resolvedPath: string,
  headers: Record<string, string>,
  body?: string,
): string {
  const fullUrl = `${window.location.origin}${resolvedPath}`
  const parts: string[] = [`curl -X ${method}`]

  for (const [k, v] of Object.entries(headers)) {
    parts.push(`  -H '${k}: ${v}'`)
  }

  if (body !== undefined && body.trim() !== '') {
    parts.push(`  -d '${body.replace(/'/g, "\\'")}'`)
  }

  parts.push(`  '${fullUrl}'`)
  return parts.join(' \\\n')
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface FormGroupProps {
  label: string
  htmlFor: string
  children: React.ReactNode
}

function FormGroup({ label, htmlFor, children }: FormGroupProps): React.JSX.Element {
  return (
    <div className="form-group">
      <label className="form-label" htmlFor={htmlFor}>
        {label}
      </label>
      {children}
    </div>
  )
}

// ---------------------------------------------------------------------------
// CurlDisplay
// ---------------------------------------------------------------------------

interface CurlDisplayProps {
  curlCommand: string
}

function CurlDisplay({ curlCommand }: CurlDisplayProps): React.JSX.Element {
  const [copied, setCopied] = useState(false)

  function handleCopy(): void {
    navigator.clipboard.writeText(curlCommand).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }).catch(() => {
      // Clipboard not available (e.g. non-secure context) — silently ignore.
    })
  }

  return (
    <div className="curl-display">
      <div className="curl-display__header">
        <span className="curl-display__label">cURL</span>
        <button
          className="curl-display__copy"
          onClick={handleCopy}
          aria-label="Copy cURL command to clipboard"
        >
          {copied ? 'Copied!' : 'Copy'}
        </button>
      </div>
      <pre className="curl-display__code">{curlCommand}</pre>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Response state
// ---------------------------------------------------------------------------

interface ResponseState {
  status: number
  statusText: string
  body: string
  latencyMs: number
}

// ---------------------------------------------------------------------------
// HttpEndpointCard
// ---------------------------------------------------------------------------

interface HttpEndpointCardProps {
  endpoint: EndpointMeta
}

/**
 * Interactive card for a single HTTP endpoint.
 *
 * Features:
 * - Collapsible via header click
 * - Auto-detected path parameters with individual inputs
 * - Global auth from AuthContext (Bearer / API Key / Basic — no per-card token)
 * - JSON body textarea pre-filled with an example skeleton (POST/PUT/PATCH/DELETE)
 * - Live cURL preview that updates as inputs change
 * - Execute button with loading state
 * - Colour-coded response panel with latency and pretty-printed body
 */
export function HttpEndpointCard({ endpoint }: HttpEndpointCardProps): React.JSX.Element {
  const uid = useId()
  const method = (endpoint.method ?? 'GET').toUpperCase()
  const pathParams = extractPathParams(endpoint.path)
  const hasBody = BODY_METHODS.has(method)

  const { auth } = useAuth()

  const [expanded, setExpanded] = useState(false)
  const [paramValues, setParamValues] = useState<Record<string, string>>(
    Object.fromEntries(pathParams.map((p) => [p, ''])),
  )
  // Pre-fill body from request_schema when available; fall back to the generic skeleton.
  const [body, setBody] = useState(() => {
    if (hasBody && endpoint.request_schema) {
      return schemaExampleJson(endpoint.request_schema as Record<string, unknown>)
    }
    return hasBody ? generateExampleBody(endpoint) : ''
  })
  const [loading, setLoading] = useState(false)
  const [response, setResponse] = useState<ResponseState | null>(null)
  const [fetchError, setFetchError] = useState<string | null>(null)

  function handleParamChange(param: string, value: string): void {
    setParamValues((prev) => ({ ...prev, [param]: value }))
  }

  // Computed values — derived from current state on every render (no stale
  // state required; React Compiler handles the memoization automatically).
  const resolvedPath = resolvePath(endpoint.path, paramValues)
  const authHeaders = buildAuthHeaders(auth)
  const requestHeaders: Record<string, string> = {
    'Content-Type': 'application/json',
    ...authHeaders,
  }
  const curlCommand = buildCurl(
    method,
    resolvedPath,
    requestHeaders,
    hasBody ? body : undefined,
  )

  async function handleExecute(): Promise<void> {
    setLoading(true)
    setResponse(null)
    setFetchError(null)

    const start = performance.now()
    try {
      const fetchInit: RequestInit = {
        method,
        headers: requestHeaders,
      }
      if (hasBody && body.trim()) {
        fetchInit.body = body
      }
      const res = await fetch(resolvedPath, fetchInit)

      const latencyMs = Math.round(performance.now() - start)
      const rawBody = await res.text()

      setResponse({
        status: res.status,
        statusText: res.statusText,
        body: prettyPrint(rawBody),
        latencyMs,
      })
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Network error'
      setFetchError(msg)
    } finally {
      setLoading(false)
    }
  }

  const headerId = `http-card-header-${uid}`
  const bodyId = `http-card-body-${uid}`

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
        <span className={`method-badge method-badge--${method.toLowerCase()}`}>
          {method}
        </span>
        <code className="endpoint-card__path">{endpoint.path}</code>
        {endpoint.description !== undefined && (
          <span className="endpoint-card__summary">{endpoint.description}</span>
        )}
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
                <FormGroup key={param} label={param} htmlFor={`${uid}-param-${param}`}>
                  <input
                    id={`${uid}-param-${param}`}
                    className="form-input"
                    type="text"
                    placeholder={param}
                    value={paramValues[param] ?? ''}
                    onChange={(e) => handleParamChange(param, e.target.value)}
                  />
                </FormGroup>
              ))}
            </section>
          )}

          {/* Auth info — read-only display of what's configured globally */}
          {auth.type !== 'none' && (
            <section className="form-section">
              <h3 className="form-section__title">
                <span className="form-section__badge">Authorization</span>
              </h3>
              <p className="auth-info-label">
                Using global auth: <strong>{auth.type}</strong>
                {auth.type === 'api-key' && auth.apiKeyHeader
                  ? ` (${auth.apiKeyHeader})`
                  : ''}
              </p>
            </section>
          )}

          {/* Request body */}
          {hasBody && (
            <section className="form-section">
              <h3 className="form-section__title">
                <span className="form-section__badge">Request Body</span>
              </h3>
              <FormGroup label="JSON" htmlFor={`${uid}-body`}>
                <textarea
                  id={`${uid}-body`}
                  className="form-textarea"
                  placeholder={'{\n  \n}'}
                  rows={6}
                  value={body}
                  onChange={(e) => setBody(e.target.value)}
                  spellCheck={false}
                />
              </FormGroup>
            </section>
          )}

          {/* Execute */}
          <div className="form-section">
            <button
              className="btn btn--primary"
              onClick={() => void handleExecute()}
              disabled={loading}
              aria-busy={loading}
            >
              {loading ? 'Executing…' : 'Execute'}
            </button>
          </div>

          {/* cURL display — always visible once the card is open */}
          <CurlDisplay curlCommand={curlCommand} />

          {/* Network error */}
          {fetchError !== null && (
            <div className="response-panel response-panel--error" role="alert">
              <p>{fetchError}</p>
            </div>
          )}

          {/* Response */}
          {response !== null && (
            <div className="response-panel">
              <div className="response-panel__meta">
                <span
                  className={`status-badge status-badge--${statusBadgeModifier(response.status)}`}
                >
                  {response.status} {response.statusText}
                </span>
                <span className="response-latency">{response.latencyMs} ms</span>
              </div>
              <pre className="response-body">
                <code>{response.body}</code>
              </pre>
            </div>
          )}

          {/* Response schema viewer */}
          {response !== null && endpoint.response_schema && (
            <section className="form-section">
              <SchemaViewer
                schema={endpoint.response_schema as Record<string, unknown>}
                label="Response"
              />
            </section>
          )}
        </div>
      )}
    </li>
  )
}
