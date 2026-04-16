/**
 * AuthModal — global authentication configuration modal.
 *
 * Presents four tabs (None / Bearer Token / API Key / Basic Auth) and
 * persists the selection via AuthContext on "Apply & Close".
 */

import { useState } from 'react'
import { useAuth, type AuthState, type AuthType } from '../context/AuthContext'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface AuthModalProps {
  onClose: () => void
}

interface TabDef {
  id: AuthType
  label: string
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TABS: TabDef[] = [
  { id: 'none', label: 'None' },
  { id: 'bearer', label: 'Bearer Token' },
  { id: 'api-key', label: 'API Key' },
  { id: 'basic', label: 'Basic Auth' },
]

// ---------------------------------------------------------------------------
// AuthModal
// ---------------------------------------------------------------------------

/**
 * Full-screen overlay modal for configuring global request authentication.
 *
 * The component keeps a local draft of the auth state and only commits it to
 * the context when the user clicks "Apply & Close", so cancelling (clicking
 * the overlay or the × button) discards uncommitted changes.
 */
export function AuthModal({ onClose }: AuthModalProps): React.JSX.Element {
  const { auth, setAuth } = useAuth()

  // Work on a local draft so closing without applying discards changes.
  const [draft, setDraft] = useState<AuthState>({ ...auth })

  function handleTabChange(type: AuthType): void {
    setDraft((prev) => ({ ...prev, type }))
  }

  function handleField<K extends keyof AuthState>(key: K, value: AuthState[K]): void {
    setDraft((prev) => ({ ...prev, [key]: value }))
  }

  function handleApply(): void {
    setAuth(draft)
    onClose()
  }

  function handleOverlayClick(e: React.MouseEvent<HTMLDivElement>): void {
    // Only close when clicking the backdrop, not the dialog itself.
    if (e.target === e.currentTarget) onClose()
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLDivElement>): void {
    if (e.key === 'Escape') onClose()
  }

  return (
    <div
      className="auth-modal-overlay"
      role="dialog"
      aria-modal="true"
      aria-label="Configure authorization"
      onClick={handleOverlayClick}
      onKeyDown={handleKeyDown}
    >
      <div className="auth-modal">
        {/* ── Header ─────────────────────────────────────────────────── */}
        <div className="auth-modal__header">
          <h2 className="auth-modal__title">Authorization</h2>
          <button
            className="auth-modal__close"
            aria-label="Close authorization dialog"
            onClick={onClose}
          >
            ✕
          </button>
        </div>

        {/* ── Type tabs ──────────────────────────────────────────────── */}
        <div className="auth-modal__tabs" role="tablist" aria-label="Auth type">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              role="tab"
              aria-selected={draft.type === tab.id}
              className={[
                'auth-tab',
                draft.type === tab.id ? 'auth-tab--active' : '',
              ]
                .filter(Boolean)
                .join(' ')}
              onClick={() => handleTabChange(tab.id)}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {/* ── Tab panels ─────────────────────────────────────────────── */}

        {draft.type === 'none' && (
          <p className="auth-modal__hint">
            No authentication will be added to requests.
          </p>
        )}

        {draft.type === 'bearer' && (
          <div className="form-group">
            <label className="form-label" htmlFor="auth-bearer">
              Bearer Token
            </label>
            <input
              id="auth-bearer"
              className="form-input"
              type="password"
              placeholder="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
              value={draft.bearer}
              onChange={(e) => handleField('bearer', e.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
            <span className="auth-modal__field-hint">
              Adds <code>Authorization: Bearer &lt;token&gt;</code> to every HTTP request.
            </span>
          </div>
        )}

        {draft.type === 'api-key' && (
          <div>
            <div className="form-group">
              <label className="form-label" htmlFor="auth-apikey-header">
                Header Name
              </label>
              <input
                id="auth-apikey-header"
                className="form-input"
                type="text"
                placeholder="X-API-Key"
                value={draft.apiKeyHeader}
                onChange={(e) => handleField('apiKeyHeader', e.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
            <div className="form-group">
              <label className="form-label" htmlFor="auth-apikey-value">
                API Key
              </label>
              <input
                id="auth-apikey-value"
                className="form-input"
                type="password"
                placeholder="sk-..."
                value={draft.apiKey}
                onChange={(e) => handleField('apiKey', e.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
          </div>
        )}

        {draft.type === 'basic' && (
          <div>
            <div className="form-group">
              <label className="form-label" htmlFor="auth-basic-user">
                Username
              </label>
              <input
                id="auth-basic-user"
                className="form-input"
                type="text"
                placeholder="admin"
                value={draft.basicUser}
                onChange={(e) => handleField('basicUser', e.target.value)}
                autoComplete="username"
              />
            </div>
            <div className="form-group">
              <label className="form-label" htmlFor="auth-basic-pass">
                Password
              </label>
              <input
                id="auth-basic-pass"
                className="form-input"
                type="password"
                placeholder="••••••••"
                value={draft.basicPass}
                onChange={(e) => handleField('basicPass', e.target.value)}
                autoComplete="current-password"
              />
            </div>
          </div>
        )}

        {/* ── Apply button ───────────────────────────────────────────── */}
        <div className="auth-modal__actions">
          <button className="btn btn--primary" onClick={handleApply}>
            Apply &amp; Close
          </button>
        </div>
      </div>
    </div>
  )
}
