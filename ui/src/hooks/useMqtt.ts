import { useEffect, useRef, useState, useCallback } from 'react'
import mqtt, { type MqttClient, type IClientOptions } from 'mqtt'

/** Default QoS level used for subscriptions and publications. */
const DEFAULT_QOS = 1 as const

/** Maximum number of messages retained in the buffer. Acts as a ring buffer
 *  cap to prevent unbounded memory growth on high-frequency topics. */
const MAX_MESSAGES = 200 as const

/** URL schemes accepted as valid MQTT broker addresses. */
const ALLOWED_BROKER_SCHEMES = ['ws:', 'wss:'] as const

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
interface UseMqttResult {
  client: MqttClient | null
  messages: string[]
  connected: boolean
  subscribe: (topic: string) => void
  publish: (topic: string, payload: string) => void
}

/**
 * Wraps the mqtt.js client lifecycle for use inside React components.
 *
 * Connects to `brokerUrl` on mount, surfaces incoming messages, and
 * disconnects cleanly when the component unmounts or `brokerUrl` changes.
 *
 * @param brokerUrl - Full broker URL, e.g. `"ws://localhost:1883"`.
 */
export function useMqtt(brokerUrl: string): UseMqttResult {
  const [connected, setConnected] = useState<boolean>(false)
  const [messages, setMessages] = useState<string[]>([])
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
      reconnectPeriod: 2000,
    }

    const mqttClient = mqtt.connect(brokerUrl, options)
    clientRef.current = mqttClient

    mqttClient.on('connect', () => {
      setConnected(true)
    })

    mqttClient.on('message', (_topic: string, payload: Buffer) => {
      const text = payload.toString('utf8')
      // Ring-buffer cap: keep only the last MAX_MESSAGES entries to prevent
      // unbounded memory growth on high-frequency MQTT topics.
      setMessages((previous) => [...previous.slice(-(MAX_MESSAGES - 1)), text])
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
    publish,
  }
}
