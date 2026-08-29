/**
 * Session + authorization navigation logic (framework-agnostic helpers).
 *
 * The server is the only enforcement point (capability guards + fresh
 * profiles/memberships per request). Everything here is the client-side UX
 * mirror: it decides what to *show* and where to *navigate*, never what is
 * true.
 *
 * Rules this module encodes:
 *  - the session comes only from `GET /v1/me` — never from stored claims;
 *  - a 401 converges on /login, but only when a session actually existed,
 *    so the /me bootstrap's own 401 (simply not logged in) and the login
 *    page don't loop;
 *  - wings are authorization domains: the admin wing needs the admins
 *    profile, the merchant wing needs a merchant membership. An account
 *    with both lands on its admin home (the rarer grant wins the default).
 */
import { redirect } from '@tanstack/react-router'
import type { QueryClient } from '@tanstack/react-query'

import { meOptions, meQueryKey } from '@/api/queries'
import {
  isMerchantOperator,
  isPlatformAdmin,
  type SessionUser,
} from '@/types/session'

export const sessionQueryKey = () => meQueryKey()

export const sessionQueryOptions = () => ({
  ...meOptions(),
  // retry is off so a 401 settles immediately instead of hammering the
  // server; staleTime is infinite because /me changes only via our own
  // mutations (login/logout/profile edits invalidate it explicitly).
  retry: false,
  staleTime: Infinity,
})

export type Wing = 'admin' | 'merchant'

/** Where a given account lands after login. Unknown accounts go to /login. */
export function homeForUser(
  user: SessionUser | null | undefined,
): '/admin' | '/merchant' | '/login' {
  if (isPlatformAdmin(user)) return '/admin'
  if (isMerchantOperator(user)) return '/merchant'
  return '/login'
}

/**
 * Route guard for `beforeLoad`. Resolves the session from the query cache
 * (fetching once if needed) and redirects:
 *   - not authenticated                  -> /login
 *   - authenticated without the wing     -> that account's home
 * On success it exposes the user on the route context for shells.
 */
export const requireWing =
  (wing: Wing) =>
  async ({ context }: { context: { queryClient: QueryClient } }) => {
    let user: SessionUser | undefined
    try {
      user = await context.queryClient.ensureQueryData(sessionQueryOptions())
    } catch {
      throw redirect({ to: '/login' })
    }
    const allowed =
      wing === 'admin' ? isPlatformAdmin(user) : isMerchantOperator(user)
    if (!user || !allowed) {
      throw redirect({ to: homeForUser(user) })
    }
    return { sessionUser: user }
  }
