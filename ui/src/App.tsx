import { useState } from 'react'
import './App.css'
import { useSpec } from './hooks/useSpec'
import { HttpPanel } from './panels/HttpPanel'
import { WsPanel } from './panels/WsPanel'
import { MqttPanel } from './panels/MqttPanel'
import type { Protocol } from './types'

/** The three protocol tabs available in the Lucy UI. */
type ActiveTab = 'http' | 'ws' | 'mqtt'

/** Maps an `ActiveTab` value to the corresponding `Protocol` discriminant. */
const TAB_TO_PROTOCOL: Record<ActiveTab, Protocol> = {
  http: 'Http',
  ws: 'WebSocket',
  mqtt: 'Mqtt',
}

/** Human-readable labels for each tab button. */
const TAB_LABELS: Record<ActiveTab, string> = {
  http: 'HTTP',
  ws: 'WebSocket',
  mqtt: 'MQTT',
}

const TABS: ActiveTab[] = ['http', 'ws', 'mqtt']

/**
 * Root application component for Lucy.
 *
 * Fetches the API spec from the Rust/Axum backend, splits endpoints by
 * protocol, and renders the appropriate panel based on the active tab.
 */
function App(): React.JSX.Element {
  const [activeTab, setActiveTab] = useState<ActiveTab>('http')
  const { spec, loading, error } = useSpec()

  const filteredEndpoints =
    spec?.endpoints.filter(
      (endpoint) => endpoint.protocol === TAB_TO_PROTOCOL[activeTab],
    ) ?? []

  return (
    <div className="app">
      <header className="app__header">
        <h1 className="app__title">Lucy</h1>
        {spec !== null && (
          <p className="app__version">Spec version: {spec.version}</p>
        )}
      </header>

      <nav aria-label="Protocol tabs">
        <ul className="tab-bar" role="tablist">
          {TABS.map((tab) => (
            <li key={tab} role="presentation">
              <button
                role="tab"
                aria-selected={activeTab === tab}
                aria-controls={`panel-${tab}`}
                id={`tab-${tab}`}
                className={[
                  'tab-bar__button',
                  activeTab === tab ? 'tab-bar__button--active' : '',
                ]
                  .filter(Boolean)
                  .join(' ')}
                onClick={() => setActiveTab(tab)}
              >
                {TAB_LABELS[tab]}
              </button>
            </li>
          ))}
        </ul>
      </nav>

      <main
        id={`panel-${activeTab}`}
        role="tabpanel"
        aria-labelledby={`tab-${activeTab}`}
        className="panel"
      >
        {loading && (
          <p className="panel-loading" aria-live="polite">
            Loading spec...
          </p>
        )}

        {!loading && error !== null && (
          <p className="panel-error" role="alert">
            {error}
          </p>
        )}

        {!loading && error === null && activeTab === 'http' && (
          <HttpPanel endpoints={filteredEndpoints} />
        )}

        {!loading && error === null && activeTab === 'ws' && (
          <WsPanel endpoints={filteredEndpoints} />
        )}

        {!loading && error === null && activeTab === 'mqtt' && (
          <MqttPanel endpoints={filteredEndpoints} />
        )}
      </main>
    </div>
  )
}

export default App
