import { useEffect, useState } from 'react'
import type { ApiSpec } from '../types'

/** URL of the spec document served by the Rust/Axum backend. */
const SPEC_URL = '/docs/spec.json' as const

/** Shape returned by the `useSpec` hook. */
interface UseSpecResult {
  spec: ApiSpec | null
  loading: boolean
  error: string | null
}

/**
 * Fetches the API specification from `SPEC_URL` on mount and returns the
 * parsed `ApiSpec` alongside loading/error state.
 *
 * The fetch is performed once per component mount. If the component unmounts
 * before the request completes the state update is suppressed to avoid
 * memory-leak warnings.
 */
export function useSpec(): UseSpecResult {
  const [spec, setSpec] = useState<ApiSpec | null>(null)
  const [loading, setLoading] = useState<boolean>(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false

    async function fetchSpec(): Promise<void> {
      try {
        const response = await fetch(SPEC_URL)

        if (!response.ok) {
          throw new Error(
            `Failed to fetch spec: ${response.status} ${response.statusText}`,
          )
        }

        const data = (await response.json()) as ApiSpec

        if (!cancelled) {
          setSpec(data)
        }
      } catch (err) {
        if (!cancelled) {
          const message =
            err instanceof Error ? err.message : 'Unknown error while fetching spec'
          setError(message)
        }
      } finally {
        if (!cancelled) {
          setLoading(false)
        }
      }
    }

    void fetchSpec()

    return () => {
      cancelled = true
    }
  }, [])

  return { spec, loading, error }
}
