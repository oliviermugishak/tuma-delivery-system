/**
 * Session + RBAC navigation logic (framework-agnostic helpers).
 *
 * The server is the only enforcement point (require_role + a fresh user
 * lookup per request). Everything here is the client-side UX mirror: it
 * decides what to *show* and where to *navigate*, never what is true.
 *
 * Rules this module encodes:
 *  - the session comes only from `GET /v1/me` — never from stored claims;
 *  - a 401 converges on /login, but only when a session actually existed,
 *    so the /me bootstrap's own 401 (simply not logged in) and the login
 *    page don't loop.
 */
import { redirect } from '@tanstack/react-router'
import type { QueryClient } from '@tanstack/react-query'

import { meOptions, meQueryKey } from '@/api/queries'
import type { Role, SessionUser } from '@/types/session'

export const sessionQueryKey = () => meQueryKey()

export const sessionQueryOptions = () => ({
  ...meOptions(),
  // retry is off so a 401 settles immediately instead of hammering the
  // server; staleTime is infinite because /me changes only via our own
  // mutations (login/logout/profile edits invalidate it explicitly).
  retry: false,
  staleTime: Infinity,
})

/** Where a given role lands after login. Unknown/absent roles go to /login. */
export function homeForRole(
  role: string | null | undefined,
): '/admin' | '/merchant' | '/login' {
  if (role === 'admin') return '/admin'
  if (role === 'merchant') return '/merchant'
  return '/login'
}

/**
 * Route guard for `beforeLoad`. Resolves the session from the query cache
 * (fetching once if needed) and redirects:
 *   - not authenticated            -> /login
 *   - authenticated, wrong wing    -> that role's home
 * On success it exposes the user on the route context for shells/topbars.
 */
export const requireRole =
  (...allowed: Role[]) =>
  async ({ context }: { context: { queryClient: QueryClient } }) => {
    let user: SessionUser | undefined
    try {
      user = await context.queryClient.ensureQueryData(sessionQueryOptions())
    } catch {
      throw redirect({ to: '/login' })
    }
    if (!user?.role) throw redirect({ to: '/login' })
    if (allowed.length > 0 && !allowed.includes(user.role as Role)) {
      throw redirect({ to: homeForRole(user.role) })
    }
    return { sessionUser: user }
  }
