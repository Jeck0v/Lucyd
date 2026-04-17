import { useState } from 'react'
import type { EndpointMeta } from '../types'
import { WsEndpointCard } from '../components/WsEndpointCard'
import { groupByTag } from '../utils/groupByTag'

/** Props accepted by the WsPanel component. */
interface WsPanelProps {
  endpoints: EndpointMeta[]
}

// ---------------------------------------------------------------------------
// EndpointGroup — collapsible section for a single tag
// ---------------------------------------------------------------------------

interface EndpointGroupProps {
  tag: string
  endpoints: EndpointMeta[]
}

function EndpointGroup({ tag, endpoints }: EndpointGroupProps): React.JSX.Element {
  const [open, setOpen] = useState(true)

  return (
    <section className="endpoint-group">
      <button
        className={[
          'endpoint-group__header',
          open ? 'endpoint-group__header--open' : '',
        ]
          .filter(Boolean)
          .join(' ')}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="endpoint-group__tag">{tag}</span>
        <span className="endpoint-group__count">{endpoints.length}</span>
        <span className="endpoint-group__chevron" aria-hidden="true">▼</span>
      </button>

      {open && (
        <div className="endpoint-group__body">
          <ul className="endpoint-list">
            {endpoints.map((endpoint) => (
              <WsEndpointCard key={endpoint.path} endpoint={endpoint} />
            ))}
          </ul>
        </div>
      )}
    </section>
  )
}

// ---------------------------------------------------------------------------
// WsPanel
// ---------------------------------------------------------------------------

/**
 * Renders WebSocket endpoint cards grouped by their first tag.
 *
 * Each card manages its own independent WebSocket connection via the
 * `useWebSocket` hook, allowing multiple endpoints to be connected
 * simultaneously. Groups are collapsible and open by default.
 */
export function WsPanel({ endpoints }: WsPanelProps): React.JSX.Element {
  if (endpoints.length === 0) {
    return (
      <p className="panel-empty">No WebSocket endpoints defined in the spec.</p>
    )
  }

  const groups = groupByTag(endpoints)

  return (
    <div>
      {[...groups.entries()].map(([tag, tagEndpoints]) => (
        <EndpointGroup key={tag} tag={tag} endpoints={tagEndpoints} />
      ))}
    </div>
  )
}
