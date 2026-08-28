/**
 * Session hooks (global). The non-hook logic (query options, guards, role
 * routing) lives in `lib/session.ts`; these are the React bindings.
 */
import { useEffect } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'

import { onSessionDeath } from '@/api/client'
import { sessionQueryKey, sessionQueryOptions } from '@/lib/session'

/** Read the current session from the query cache (no refetch by default). */
export function useSession() {
  return useQuery(sessionQueryOptions())
}

/**
 * Global 401 convergence. Mounted once near the root. When any request dies
 * with 401 and a session existed, clear the cache and go to /login. Acting
 * only when a session was cached keeps the /me bootstrap's own 401 (simply
 * not logged in) and the login page from looping.
 */
export function useSessionDeathWatcher() {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  useEffect(() => {
    onSessionDeath(() => {
      const hadSession = queryClient.getQueryData(sessionQueryKey())
      if (hadSession) {
        queryClient.clear()
        void navigate({ to: '/login' })
      }
    })
    return () => onSessionDeath(null)
  }, [queryClient, navigate])
}
