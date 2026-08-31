import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { listRidersQueryKey, updateRiderMutation } from '@/api/queries'

/**
 * Edit a rider (admin-only): name, phone, and the assignable flag all ride
 * the same PATCH. A phone edit moves the OTP anchor too — the remedy for a
 * typo'd number at creation. A taken phone surfaces the server's 409
 * message. Toast on both outcomes and refetch the list.
 */
export function useUpdateRider() {
  const queryClient = useQueryClient()
  return useMutation({
    ...updateRiderMutation(),
    onSuccess: (rider, variables) => {
      if (variables.body?.is_active !== undefined) {
        toast.success(
          rider.is_active
            ? `${rider.name} is now assignable`
            : `${rider.name} is no longer assignable`,
        )
      } else {
        toast.success(`${rider.name} updated`)
      }
      void queryClient.invalidateQueries({ queryKey: listRidersQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not update the rider',
      )
    },
  })
}
