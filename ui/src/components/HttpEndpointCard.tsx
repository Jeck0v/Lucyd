import { useState, useId } from 'react'
import type { EndpointMeta } from '../types'
import { useAuth, buildAuthHeaders } from '../context/AuthContext'
import { SchemaViewer } from './SchemaViewer'
import { schemaExampleJson } from '../utils/schemaToExample'
import { appendQueryString, extractQueryParams } from '../utils/queryParams'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** HTTP methods that carry a request body. */
const BODY_METHODS = new Set(['POST', 'PUT', 'PATCH', 'DELETE'])

// ---------------------------------------------------------------------------
// JSON syntax highlighter
// ---------------------------------------------------------------------------

type JsonToken =
  | { kind: 'key';     text: string }
  | { kind: 'string';  text: string }
  | { kind: 'number';  text: string }
  | { kind: 'boolean'; text: string }
  | { kind: 'null' }
  | { kind: 'brace';   text: string }
  | { kind: 'raw';     text: string }

/**
 * Tokenises a pretty-printed JSON string into typed tokens.
 * Uses `String.prototype.matchAll` — no exec() involved.
 */
function tokeniseJson(src: string): JsonToken[] {
  const TOKEN_RE =
    /("(?:[^"\\]|\\.)*")(\s*:)?|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|(\btrue\b|\bfalse\b)|\bnull\b|([{}[\],])/g

  const tokens: JsonToken[] = []
  let cursor = 0

  for (const m of src.matchAll(TOKEN_RE)) {
    const at = m.index ?? 0
    if (at > cursor) {
      tokens.push({ kind: 'raw', text: src.slice(cursor, at) })
    }

    const [full, strPart, colon, numPart, boolPart, bracePart] = m

    if (strPart !== undefined) {
      if (colon !== undefined) {
        tokens.push({ kind: 'key', text: strPart + colon })
      } else {
        tokens.push({ kind: 'string', text: strPart })
      }
    } else if (numPart !== undefined) {
      tokens.push({ kind: 'number', text: numPart })
    } else if (boolPart !== undefined) {
      tokens.push({ kind: 'boolean', text: boolPart })
    } else if (full === 'null') {
      tokens.push({ kind: 'null' })
    } else if (bracePart !== undefined) {
      tokens.push({ kind: 'brace', text: bracePart })
    } else {
      tokens.push({ kind: 'raw', text: full })
    }

    cursor = at + full.length
  }

  if (cursor < src.length) {
    tokens.push({ kind: 'raw', text: src.slice(cursor) })
  }

  return tokens
}

/**
 * Applies basic JSON syntax highlighting. Returns React nodes with
 * colour-coded `<span>` elements. Falls back to plain text for non-JSON.
 */
function highlightJson(raw: string): React.ReactNode {
  const trimmed = raw.trim()
  if (!trimmed.startsWith('{') && !trimmed.startsWith('[')) {
    return raw
  }

  return tokeniseJson(raw).map((token, i) => {
    switch (token.kind) {
      case 'key':     return <span key={i} className="json-key">{token.text}</span>
      case 'string':  return <span key={i} className="json-string">{token.text}</span>
      case 'number':  return <span key={i} className="json-number">{token.text}</span>
      case 'boolean': return <span key={i} className="json-boolean">{token.text}</span>
      case 'null':    return <span key={i} className="json-null">null</span>
      case 'brace':   return <span key={i} className="json-brace">{token.text}</span>
      default:        return token.text
    }
  })
}

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
 * host as the Lucyd UI, which is co-located with the Axum backend.
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
// ParamRow — one row of the Parameters table
// ---------------------------------------------------------------------------

interface ParamRowProps {
  uid: string
  name: string
  /** Where the value goes: into the path template, or the query string. */
  location: 'path' | 'query'
  type: string
  required: boolean
  description: string | undefined
  value: string
  onChange: (value: string) => void
}

/**
 * Renders one parameter, whatever its location: path parameters are always
 * required strings, query parameters carry the type and required-ness their
 * schema declared.
 */
function ParamRow({
  uid, name, location, type, required, description, value, onChange,
}: ParamRowProps): React.JSX.Element {
  return (
    <tr>
      <td>
        <span className="param-name">
          {name}
          {required && <span className="param-required" aria-label="required">*</span>}
        </span>
        {description !== undefined && <p className="param-description">{description}</p>}
      </td>
      <td>
        <span className="param-type">{type}</span>
        <span className="param-in">({location})</span>
      </td>
      <td>
        <input
          id={`${uid}-${location}-param-${name}`}
          className="form-input"
          type="text"
          placeholder={`Enter ${name}`}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-label={`${location === 'path' ? 'Path' : 'Query'} parameter: ${name}`}
        />
      </td>
    </tr>
  )
}

// ---------------------------------------------------------------------------
// RequestBodyEditor — Swagger-style request body section
// ---------------------------------------------------------------------------

interface RequestBodyEditorProps {
  uid: string
  body: string
  schema: Record<string, unknown> | undefined
  onChange: (value: string) => void
}

function RequestBodyEditor({
  uid,
  body,
  schema,
  onChange,
}: RequestBodyEditorProps): React.JSX.Element {
  const [tab, setTab] = useState<'editor' | 'schema'>('editor')

  return (
    <div className="request-body">
      {/* Header bar */}
      <div className="request-body__header">
        <span className="request-body__title">Request body</span>
        <span className="request-body__required" aria-label="required">*</span>
        <span className="request-body__content-type">application/json</span>
      </div>

      {/* Tabs — only show Schema tab when a schema is available */}
      {schema !== undefined && (
        <div className="request-body__tabs" role="tablist">
          <button
            role="tab"
            aria-selected={tab === 'editor'}
            className={`request-body__tab${tab === 'editor' ? ' request-body__tab--active' : ''}`}
            onClick={() => setTab('editor')}
          >
            Example Value
          </button>
          <button
            role="tab"
            aria-selected={tab === 'schema'}
            className={`request-body__tab${tab === 'schema' ? ' request-body__tab--active' : ''}`}
            onClick={() => setTab('schema')}
          >
            Schema
          </button>
        </div>
      )}

      {/* Content */}
      <div className="request-body__editor">
        {(tab === 'editor' || schema === undefined) ? (
          <textarea
            id={`${uid}-body`}
            className="request-body__textarea"
            placeholder={'{\n  \n}'}
            value={body}
            onChange={(e) => onChange(e.target.value)}
            spellCheck={false}
            aria-label="Request body JSON"
          />
        ) : (
          <SchemaViewer schema={schema} label="Request" />
        )}
      </div>
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
 * - Collapsible via header click with Swagger-style method-coloured header
 * - Auto-detected path parameters rendered as a table with individual inputs
 * - Global auth from AuthContext (Bearer / API Key / Basic)
 * - JSON body editor with content-type badge and optional Schema tab
 * - Live cURL preview that updates as inputs change
 * - Prominent Execute button coloured by HTTP method with loading spinner
 * - Colour-coded response panel with JSON syntax highlighting
 */
export function HttpEndpointCard({ endpoint }: HttpEndpointCardProps): React.JSX.Element {
  const uid = useId()
  const method = (endpoint.method ?? 'GET').toUpperCase()
  const pathParams = extractPathParams(endpoint.path)
  const queryParams = extractQueryParams(endpoint.query_schema)
  const hasBody = BODY_METHODS.has(method)

  const { auth } = useAuth()

  const [expanded, setExpanded] = useState(false)
  const [paramValues, setParamValues] = useState<Record<string, string>>(
    Object.fromEntries(pathParams.map((p) => [p, ''])),
  )
  const [queryValues, setQueryValues] = useState<Record<string, string>>(
    Object.fromEntries(queryParams.map((p) => [p.name, ''])),
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

  function handleQueryChange(param: string, value: string): void {
    setQueryValues((prev) => ({ ...prev, [param]: value }))
  }

  // Computed values — derived from current state on every render.
  // The query string is appended once, here, so `fetch` and the cURL preview
  // are the same URL by construction rather than by two matching edits.
  const resolvedPath = appendQueryString(
    resolvePath(endpoint.path, paramValues),
    Object.entries(queryValues),
  )
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

  const requestSchema = endpoint.request_schema as Record<string, unknown> | undefined

  return (
    <li className="endpoint-card">
      {/* ── Header (Swagger-style toggle row) ────────────────────────── */}
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
        <span
          className={`endpoint-card__chevron${expanded ? ' endpoint-card__chevron--open' : ''}`}
          aria-hidden="true"
        >
          ›
        </span>
      </button>

      {/* ── Expandable body ──────────────────────────────────────────── */}
      {expanded && (
        <div id={bodyId} className="endpoint-card__body" role="region" aria-labelledby={headerId}>

          {/* Path and query parameters — table layout */}
          {pathParams.length + queryParams.length > 0 && (
            <div className="endpoint-section">
              <div className="section-header">
                <span className="section-title">Parameters</span>
              </div>
              <table className="params-table" role="table">
                <thead>
                  <tr>
                    <th scope="col">Name</th>
                    <th scope="col">Type</th>
                    <th scope="col">Value</th>
                  </tr>
                </thead>
                <tbody>
                  {pathParams.map((param) => (
                    <ParamRow
                      key={`path-${param}`}
                      uid={uid}
                      name={param}
                      location="path"
                      type="string"
                      required
                      description={undefined}
                      value={paramValues[param] ?? ''}
                      onChange={(value) => handleParamChange(param, value)}
                    />
                  ))}
                  {queryParams.map((param) => (
                    <ParamRow
                      key={`query-${param.name}`}
                      uid={uid}
                      name={param.name}
                      location="query"
                      type={param.type}
                      required={param.required}
                      description={param.description}
                      value={queryValues[param.name] ?? ''}
                      onChange={(value) => handleQueryChange(param.name, value)}
                    />
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {/* Auth info — read-only display of what's configured globally */}
          {auth.type !== 'none' && (
            <div className="endpoint-section">
              <div className="section-header">
                <span className="section-title">Authorization</span>
              </div>
              <p className="auth-info-label">
                <span>Global auth:</span>
                <strong>{auth.type}</strong>
                {auth.type === 'api-key' && auth.apiKeyHeader
                  ? <code>({auth.apiKeyHeader})</code>
                  : null}
              </p>
            </div>
          )}

          {/* Request body — Swagger-style editor */}
          {hasBody && (
            <div className="endpoint-section">
              <RequestBodyEditor
                uid={uid}
                body={body}
                schema={requestSchema}
                onChange={setBody}
              />
            </div>
          )}

          {/* Execute */}
          <div className="execute-bar">
            <button
              className="btn-execute"
              onClick={() => void handleExecute()}
              disabled={loading}
              aria-busy={loading}
            >
              {loading && <span className="btn-execute__spinner" aria-hidden="true" />}
              {loading ? 'Executing…' : 'Execute'}
            </button>
          </div>

          {/* cURL preview */}
          <CurlDisplay curlCommand={curlCommand} />

          {/* Network error */}
          {fetchError !== null && (
            <div className="response-panel response-panel--error" role="alert">
              <p>{fetchError}</p>
            </div>
          )}

          {/* Response */}
          {response !== null && (
            <div className="response-section">
              <p className="response-section__title">Server response</p>
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
                  <code>{highlightJson(response.body)}</code>
                </pre>
              </div>
            </div>
          )}

          {/* Response schema viewer */}
          {response !== null && endpoint.response_schema && (
            <div className="endpoint-section">
              <SchemaViewer
                schema={endpoint.response_schema as Record<string, unknown>}
                label="Response"
              />
            </div>
          )}
        </div>
      )}
    </li>
  )
}
