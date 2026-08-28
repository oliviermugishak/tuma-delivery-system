import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { updateMeMutation } from '@/api/queries'
import { sessionQueryKey } from '@/lib/session'

/**
 * Update the signed-in user's profile (name only in V1). The PATCH returns
 * the fresh user, so it is written straight into the session cache — the
 * topbar and the profile card pick it up without a refetch.
 */
export function useUpdateMe() {
  const queryClient = useQueryClient()
  return useMutation({
    ...updateMeMutation(),
    onSuccess: (user) => {
      toast.success('Profile updated')
      queryClient.setQueryData(sessionQueryKey(), user)
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError
          ? error.message
          : 'Could not update the profile',
      )
    },
  })
}
