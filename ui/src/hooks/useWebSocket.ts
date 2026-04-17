import { useRef, useState, useCallback } from 'react'
import type { WsMessage } from '../types'

/** Maximum messages retained in the log (ring-buffer cap). */
const MAX_MESSAGES = 200 as const

/** Monotonically-increasing counter for unique message IDs. */
let wsMessageIdCounter = 0

/** Returns a formatted `HH:mm:ss` timestamp for the current local time. */
function currentTimestamp(): string {
  const now = new Date()
  const hh = String(now.getHours()).padStart(2, '0')
  const mm = String(now.getMinutes()).padStart(2, '0')
  const ss = String(now.getSeconds()).padStart(2, '0')
  return `${hh}:${mm}:${ss}`
}

/** Connection lifecycle states for a WebSocket. */
export type WsStatus = 'disconnected' | 'connecting' | 'connected' | 'error'

/** Human-readable close reason surfaced when a connection fails or is rejected. */
export interface WsCloseInfo {
  code: number
  reason: string
}

/** Shape returned by the `useWebSocket` hook. */
export interface UseWebSocketResult {
  status: WsStatus
  messages: WsMessage[]
  lastClose: WsCloseInfo | null
  connect: (url: string) => void
  disconnect: () => void
  send: (payload: string) => void
  clearMessages: () => void
}

/**
 * Maps WebSocket close codes to human-readable labels.
 * Covers the most common codes from RFC 6455 §7.4.
 */
function describeCloseCode(code: number): string {
  switch (code) {
    case 1000: return 'Normal closure'
    case 1001: return 'Endpoint going away'
    case 1002: return 'Protocol error'
    case 1003: return 'Unsupported data type'
    case 1005: return 'No status received'
    case 1006: return 'Connection lost (abnormal)'
    case 1007: return 'Invalid data'
    case 1008: return 'Policy violation (check auth)'
    case 1009: return 'Message too large'
    case 1010: return 'Extension negotiation failed'
    case 1011: return 'Internal server error'
    case 1012: return 'Service restart'
    case 1013: return 'Try again later'
    case 1015: return 'TLS handshake failed'
    case 4000: return 'Unauthorized'
    case 4001: return 'Authentication failed'
    case 4003: return 'Forbidden'
    default:   return `Unknown (${code})`
  }
}

/**
 * Manages a single WebSocket connection imperatively.
 *
 * The caller drives the lifecycle via `connect` / `disconnect`.
 * Incoming messages are appended to `messages` with direction `'in'`;
 * outgoing messages sent via `send` are echoed with direction `'out'`.
 *
 * When a connection closes abnormally, `status` stays `'error'` (not
 * `'disconnected'`) and `lastClose` carries the close code + reason so
 * the UI can surface a meaningful error message.
 *
 * The log is capped at 200 entries to prevent unbounded memory growth.
 */
export function useWebSocket(): UseWebSocketResult {
  const [status, setStatus] = useState<WsStatus>('disconnected')
  const [messages, setMessages] = useState<WsMessage[]>([])
  const [lastClose, setLastClose] = useState<WsCloseInfo | null>(null)
  const socketRef = useRef<WebSocket | null>(null)

  const appendMessage = useCallback(
    (direction: 'in' | 'out', payload: string): void => {
      const msg: WsMessage = {
        id: ++wsMessageIdCounter,
        direction,
        payload,
        timestamp: currentTimestamp(),
      }
      setMessages((prev) => [...prev.slice(-(MAX_MESSAGES - 1)), msg])
    },
    [],
  )

  const connect = useCallback(
    (url: string): void => {
      // Close any existing socket before opening a new one.
      if (socketRef.current !== null) {
        socketRef.current.onclose = null
        socketRef.current.close()
        socketRef.current = null
      }

      setStatus('connecting')
      setLastClose(null)

      let ws: WebSocket
      try {
        ws = new WebSocket(url)
      } catch (err) {
        console.error('[useWebSocket] failed to create WebSocket:', err)
        setStatus('error')
        setLastClose({
          code: 0,
          reason: err instanceof Error ? err.message : 'Invalid WebSocket URL',
        })
        return
      }

      socketRef.current = ws

      ws.onopen = () => {
        setStatus('connected')
        setLastClose(null)
      }

      ws.onmessage = (event: MessageEvent<string>) => {
        appendMessage('in', String(event.data))
      }

      ws.onerror = () => {
        // onerror always precedes onclose — actual cleanup happens in onclose.
        // We don't set status here to avoid a double render.
      }

      ws.onclose = (event: CloseEvent) => {
        socketRef.current = null
        const reason =
          event.reason.trim() !== ''
            ? event.reason
            : describeCloseCode(event.code)

        if (event.code === 1000 || event.code === 1001) {
          // Normal / going-away close: treat as clean disconnect.
          setStatus('disconnected')
          setLastClose(null)
        } else {
          // Abnormal close (rejected, lost, auth error, …): keep 'error' state
          // so the user sees the close code rather than just a silent disconnect.
          setStatus('error')
          setLastClose({ code: event.code, reason })
        }
      }
    },
    [appendMessage],
  )

  const disconnect = useCallback((): void => {
    if (socketRef.current !== null) {
      // Remove onclose so we don't trigger the error path on a voluntary close.
      socketRef.current.onclose = null
      socketRef.current.close(1000, 'User disconnected')
      socketRef.current = null
    }
    setStatus('disconnected')
    setLastClose(null)
  }, [])

  const send = useCallback(
    (payload: string): void => {
      if (socketRef.current?.readyState === WebSocket.OPEN) {
        socketRef.current.send(payload)
        appendMessage('out', payload)
      }
    },
    [appendMessage],
  )

  const clearMessages = useCallback((): void => {
    setMessages([])
  }, [])

  return { status, messages, lastClose, connect, disconnect, send, clearMessages }
}
