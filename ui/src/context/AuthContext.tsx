/**
 * AuthContext — global authentication state for Lucy.
 *
 * Persists the selected auth type and credentials to `localStorage` so the
 * user does not have to re-enter them on page reload.
 */

import {
  createContext,
  useCallback,
  useContext,
  useState,
  type ReactNode,
} from 'react'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type AuthType = 'none' | 'bearer' | 'api-key' | 'basic'

export interface AuthState {
  type: AuthType
  /** Bearer token value */
  bearer: string
  /** Header name for API key auth, e.g. "X-API-Key" */
  apiKeyHeader: string
  /** API key value */
  apiKey: string
  basicUser: string
  basicPass: string
}

interface AuthContextValue {
  auth: AuthState
  setAuth: (next: AuthState) => void
}

// ---------------------------------------------------------------------------
// Defaults & persistence
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'lucy-auth'

const DEFAULT_AUTH: AuthState = {
  type: 'none',
  bearer: '',
  apiKeyHeader: 'X-API-Key',
  apiKey: '',
  basicUser: '',
  basicPass: '',
}

function loadFromStorage(): AuthState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw === null) return DEFAULT_AUTH
    return { ...DEFAULT_AUTH, ...(JSON.parse(raw) as Partial<AuthState>) }
  } catch {
    return DEFAULT_AUTH
  }
}

function saveToStorage(auth: AuthState): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(auth))
  } catch {
    // Storage quota exceeded or private-browsing restriction — silently ignore.
  }
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

const AuthContext = createContext<AuthContextValue | null>(null)

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

interface AuthProviderProps {
  children: ReactNode
}

export function AuthProvider({ children }: AuthProviderProps): React.JSX.Element {
  const [auth, setAuthState] = useState<AuthState>(loadFromStorage)

  const setAuth = useCallback((next: AuthState): void => {
    saveToStorage(next)
    setAuthState(next)
  }, [])

  return (
    <AuthContext.Provider value={{ auth, setAuth }}>
      {children}
    </AuthContext.Provider>
  )
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Returns the current global auth state and a setter that also persists to
 * `localStorage`.
 */
export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext)
  if (ctx === null) {
    throw new Error('useAuth must be used inside <AuthProvider>')
  }
  return ctx
}

// ---------------------------------------------------------------------------
// Utility: build HTTP headers from auth state
// ---------------------------------------------------------------------------

/**
 * Derives the HTTP Authorization / custom headers from the current auth
 * configuration. Returns an empty object when auth type is 'none'.
 */
export function buildAuthHeaders(auth: AuthState): Record<string, string> {
  switch (auth.type) {
    case 'bearer':
      if (auth.bearer.trim() === '') return {}
      return { Authorization: `Bearer ${auth.bearer.trim()}` }

    case 'api-key':
      if (auth.apiKeyHeader.trim() === '' || auth.apiKey.trim() === '') return {}
      return { [auth.apiKeyHeader.trim()]: auth.apiKey.trim() }

    case 'basic': {
      if (auth.basicUser.trim() === '') return {}
      const encoded = btoa(`${auth.basicUser}:${auth.basicPass}`)
      return { Authorization: `Basic ${encoded}` }
    }

    default:
      return {}
  }
}
