/**
 * Shared TypeScript types mirroring the Rust `EndpointMeta` struct
 * and `ApiSpec` response from the Axum backend.
 */

/** The transport protocol used by an endpoint. */
export type Protocol = 'Http' | 'WebSocket' | 'Mqtt'

/**
 * Metadata for a single endpoint as returned by `/docs/spec.json`.
 * Optional fields are absent for protocols that do not use them
 * (e.g. `method` is undefined for WebSocket and MQTT endpoints).
 */
export interface EndpointMeta {
  name: string
  path: string
  protocol: Protocol
  description?: string
  /** HTTP verb (GET, POST, etc.). Defined only for Http endpoints. */
  method?: string
  request_schema?: Record<string, unknown>
  response_schema?: Record<string, unknown>
  /** Classification tags for grouping endpoints (e.g. ["screens", "api"]). */
  tags?: string[]
}

/**
 * Top-level shape of the `/docs/spec.json` response.
 */
export interface ApiSpec {
  version: string
  endpoints: EndpointMeta[]
}

/** A single message in a WebSocket conversation log. */
export interface WsMessage {
  id: number
  /** `'in'` = received from server, `'out'` = sent by the user. */
  direction: 'in' | 'out'
  payload: string
  /** Formatted as `HH:mm:ss`. */
  timestamp: string
}

/** A single message captured on an MQTT topic. */
export interface MqttMessage {
  id: number
  topic: string
  payload: string
  /** Formatted as `HH:mm:ss`. */
  timestamp: string
}
