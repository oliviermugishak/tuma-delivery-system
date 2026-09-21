import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { me } from '@/api/generated'
import { loginMutation } from '@/api/queries'
import { homeForUser, sessionQueryKey } from '@/lib/session'

/**
 * Email + password sign-in. On success the session cookies are already set by
 * the server, so we fetch /me to learn the role, seed the session query cache
 * (so the destination wing's guard doesn't refetch), then route to the role's
 * home. Errors toast AND stay on `mutation.error` for the form.
 */
export function useLogin() {
  const queryClient = useQueryClient()
  const navigate = useNavigate()

  return useMutation({
    ...loginMutation(),
    onSuccess: async () => {
      try {
        const { data, error } = await me()
        if (error || !data) {
          toast.error(
            'Signed in, but the session did not stick. Check that the API URL is set and cookies are allowed.',
          )
          return
        }
        queryClient.setQueryData(sessionQueryKey(), data)
        toast.success('Signed in')
        void navigate({ to: homeForUser(data) })
      } catch (error) {
        toast.error(loginErrorMessage(error))
      }
    },
    onError: (error) => {
      toast.error(loginErrorMessage(error))
    },
  })
}

/** Human-readable message for a failed sign-in. */
export function loginErrorMessage(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.status === 0 || error.status >= 500) {
      return 'Cannot reach the server. Try again in a moment.'
    }
    return error.message
  }
  if (error instanceof TypeError) {
    return 'Cannot reach the server. Try again in a moment.'
  }
  return 'Sign-in failed'
}
