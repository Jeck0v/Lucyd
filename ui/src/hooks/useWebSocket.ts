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

/** Shape returned by the `useWebSocket` hook. */
export interface UseWebSocketResult {
  status: WsStatus
  messages: WsMessage[]
  connect: (url: string) => void
  disconnect: () => void
  send: (payload: string) => void
  clearMessages: () => void
}

/**
 * Manages a single WebSocket connection imperatively.
 *
 * The caller drives the lifecycle via `connect` / `disconnect`.
 * Incoming messages are appended to `messages` with direction `'in'`;
 * outgoing messages sent via `send` are echoed with direction `'out'`.
 *
 * The log is capped at 200 entries to prevent unbounded memory growth.
 */
export function useWebSocket(): UseWebSocketResult {
  const [status, setStatus] = useState<WsStatus>('disconnected')
  const [messages, setMessages] = useState<WsMessage[]>([])
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

      let ws: WebSocket
      try {
        ws = new WebSocket(url)
      } catch (err) {
        console.error('[useWebSocket] failed to create WebSocket:', err)
        setStatus('error')
        return
      }

      socketRef.current = ws

      ws.onopen = () => {
        setStatus('connected')
      }

      ws.onmessage = (event: MessageEvent<string>) => {
        appendMessage('in', String(event.data))
      }

      ws.onerror = () => {
        // `onerror` fires before `onclose`; actual cleanup happens in `onclose`.
        setStatus('error')
      }

      ws.onclose = () => {
        socketRef.current = null
        setStatus('disconnected')
      }
    },
    [appendMessage],
  )

  const disconnect = useCallback((): void => {
    if (socketRef.current !== null) {
      // Remove onclose so we don't double-update state via the event.
      socketRef.current.onclose = null
      socketRef.current.close()
      socketRef.current = null
    }
    setStatus('disconnected')
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

  return { status, messages, connect, disconnect, send, clearMessages }
}
