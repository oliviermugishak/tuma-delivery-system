import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { deleteRiderMutation, listRidersQueryKey } from '@/api/queries'

/**
 * Hard-delete a rider (admin-only). Permanent — the UI confirms in an alert
 * dialog before this ever runs; it's the remedy for a typo'd phone at
 * creation. Blocked with a 409 when the rider has delivery history — the
 * server's message ("deactivate instead") surfaces verbatim. Refetches the
 * list afterwards.
 */
export function useDeleteRider() {
  const queryClient = useQueryClient()
  return useMutation({
    ...deleteRiderMutation(),
    onSuccess: () => {
      toast.success('Rider deleted')
      void queryClient.invalidateQueries({ queryKey: listRidersQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not delete the rider',
      )
    },
  })
}
