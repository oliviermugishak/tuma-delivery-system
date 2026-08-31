import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { createRiderMutation, listRidersQueryKey } from '@/api/queries'

/**
 * Create a rider (admin-only): the OTP account + the rider profile with its
 * server-generated unique rider number, in one call. Riders never
 * self-signup — this dialog is the whole onboarding. On success: toast
 * carries the number (the merchant asks the rider for it at handoff) and
 * refetches the list. On failure: surface the server's own message (e.g.
 * the 409 "phone already taken") so the admin can fix the input and retry.
 */
export function useCreateRider() {
  const queryClient = useQueryClient()
  return useMutation({
    ...createRiderMutation(),
    onSuccess: (created) => {
      toast.success(
        `${created.name} created — rider #${created.rider_number}. They sign in in the app with their phone + OTP.`,
      )
      void queryClient.invalidateQueries({ queryKey: listRidersQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not create the rider',
      )
    },
  })
}
