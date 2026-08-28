import { useMutation } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { changePasswordMutation } from '@/api/queries'

/**
 * Change the signed-in user's password. Success toasts and the form clears
 * itself via the onSuccess callback. Failure surfaces the server's own
 * message — the common one is the 401 "current password is incorrect".
 * Note: a wrong current password does NOT kill the session — the client's
 * 401 watcher skips this endpoint (see api/client.ts).
 */
export function useChangePassword() {
  return useMutation({
    ...changePasswordMutation(),
    onSuccess: () => {
      toast.success('Password changed')
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError
          ? error.message
          : 'Could not change the password',
      )
    },
  })
}
