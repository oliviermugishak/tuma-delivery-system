import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'

import { ApiError } from '@/api/client'
import { me } from '@/api/generated'
import { loginMutation } from '@/api/queries'
import { homeForRole, sessionQueryKey } from '@/lib/session'

/**
 * Email + password sign-in. On success the session cookies are already set by
 * the server, so we fetch /me to learn the role, seed the session query cache
 * (so the destination wing's guard doesn't refetch), then route to the role's
 * home. Errors surface via `mutation.error` for the form to render.
 */
export function useLogin() {
  const queryClient = useQueryClient()
  const navigate = useNavigate()

  return useMutation({
    ...loginMutation(),
    onSuccess: async () => {
      const { data } = await me()
      if (data) queryClient.setQueryData(sessionQueryKey(), data)
      void navigate({ to: homeForRole(data?.role) })
    },
  })
}

/** Human-readable message for a failed sign-in; the server's is generic. */
export function loginErrorMessage(error: unknown): string {
  return error instanceof ApiError ? error.message : 'Sign-in failed'
}
