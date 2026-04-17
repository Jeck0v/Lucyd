import { useEffect, useRef, useState, useCallback } from 'react'
import mqtt, { type MqttClient, type IClientOptions } from 'mqtt'
import type { MqttMessage } from '../types'

/** Default QoS level used for subscriptions and publications. */
const DEFAULT_QOS = 1 as const

/** Maximum number of messages retained in the buffer. Acts as a ring buffer
 *  cap to prevent unbounded memory growth on high-frequency topics. */
const MAX_MESSAGES = 200 as const

/** URL schemes accepted as valid MQTT broker addresses. */
const ALLOWED_BROKER_SCHEMES = ['ws:', 'wss:'] as const

/** Monotonically-increasing counter used to assign unique message IDs. */
let messageIdCounter = 0

/** Returns a formatted `HH:mm:ss` timestamp for the current local time. */
function currentTimestamp(): string {
  const now = new Date()
  const hh = String(now.getHours()).padStart(2, '0')
  const mm = String(now.getMinutes()).padStart(2, '0')
  const ss = String(now.getSeconds()).padStart(2, '0')
  return `${hh}:${mm}:${ss}`
}

/**
 * Validates that `url` is a well-formed WebSocket URL.
 * Rejects non-ws/wss schemes to prevent unintended outbound connections.
 */
function isValidBrokerUrl(url: string): boolean {
  try {
    const parsed = new URL(url)
    return (ALLOWED_BROKER_SCHEMES as readonly string[]).includes(parsed.protocol)
  } catch {
    return false
  }
}

/** Shape returned by the `useMqtt` hook. */
export interface UseMqttResult {
  client: MqttClient | null
  messages: MqttMessage[]
  connected: boolean
  subscribe: (topic: string) => void
  unsubscribe: (topic: string) => void
  publish: (topic: string, payload: string) => void
}

/**
 * Wraps the mqtt.js client lifecycle for use inside React components.
 *
 * Connects to `brokerUrl` on mount (when non-empty and valid), surfaces
 * incoming messages as typed `MqttMessage` objects, and disconnects cleanly
 * when the component unmounts or `brokerUrl` changes.
 *
 * @param brokerUrl - Full broker URL, e.g. `"ws://localhost:1883"`.
 *                    Pass an empty string to stay disconnected.
 */
export function useMqtt(brokerUrl: string): UseMqttResult {
  const [connected, setConnected] = useState<boolean>(false)
  const [messages, setMessages] = useState<MqttMessage[]>([])
  const clientRef = useRef<MqttClient | null>(null)

  useEffect(() => {
    // Disconnect any existing client if the URL becomes empty or invalid.
    if (!brokerUrl || !isValidBrokerUrl(brokerUrl)) {
      if (clientRef.current) {
        clientRef.current.end(true)
        clientRef.current = null
        setConnected(false)
      }
      return
    }

    const options: IClientOptions = {
      reconnectPeriod: 0, // manual reconnect — caller drives connect/disconnect
    }

    const mqttClient = mqtt.connect(brokerUrl, options)
    clientRef.current = mqttClient

    mqttClient.on('connect', () => {
      setConnected(true)
    })

    mqttClient.on('message', (topic: string, payload: Buffer) => {
      const text = payload.toString('utf8')
      const msg: MqttMessage = {
        id: ++messageIdCounter,
        topic,
        payload: text,
        timestamp: currentTimestamp(),
      }
      setMessages((previous) => [...previous.slice(-(MAX_MESSAGES - 1)), msg])
    })

    mqttClient.on('error', (err: Error) => {
      console.error('[useMqtt] client error:', err.message)
    })

    mqttClient.on('close', () => {
      setConnected(false)
    })

    return () => {
      mqttClient.end(true)
      clientRef.current = null
      setConnected(false)
    }
  }, [brokerUrl])

  const subscribe = useCallback((topic: string): void => {
    clientRef.current?.subscribe(topic, { qos: DEFAULT_QOS }, (err) => {
      if (err) {
        console.error(`[useMqtt] subscribe error on "${topic}":`, err.message)
      }
    })
  }, [])

  const unsubscribe = useCallback((topic: string): void => {
    clientRef.current?.unsubscribe(topic, (err) => {
      if (err) {
        console.error(`[useMqtt] unsubscribe error on "${topic}":`, err?.message)
      }
    })
  }, [])

  const publish = useCallback((topic: string, payload: string): void => {
    clientRef.current?.publish(topic, payload, { qos: DEFAULT_QOS }, (err) => {
      if (err) {
        console.error(`[useMqtt] publish error on "${topic}":`, err.message)
      }
    })
  }, [])

  return {
    client: clientRef.current,
    messages,
    connected,
    subscribe,
    unsubscribe,
    publish,
  }
}
