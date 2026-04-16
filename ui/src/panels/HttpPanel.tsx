import { useState } from 'react'
import type { EndpointMeta } from '../types'
import { HttpEndpointCard } from '../components/HttpEndpointCard'
import { groupByTag } from '../utils/groupByTag'

/** Props accepted by the HttpPanel component. */
interface HttpPanelProps {
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
              <HttpEndpointCard
                key={`${endpoint.method ?? 'GET'}-${endpoint.path}`}
                endpoint={endpoint}
              />
            ))}
          </ul>
        </div>
      )}
    </section>
  )
}

// ---------------------------------------------------------------------------
// HttpPanel
// ---------------------------------------------------------------------------

/**
 * Renders HTTP endpoint cards grouped by their first tag.
 *
 * Each group is a collapsible section (open by default). Endpoints without
 * a `tags` field fall into the implicit "default" group.
 */
export function HttpPanel({ endpoints }: HttpPanelProps): React.JSX.Element {
  if (endpoints.length === 0) {
    return (
      <p className="panel-empty">No HTTP endpoints defined in the spec.</p>
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
