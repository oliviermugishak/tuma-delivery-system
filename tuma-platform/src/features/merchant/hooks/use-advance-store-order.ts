import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  listMerchantOrdersQueryKey,
  advanceStoreOrderMutation,
} from '@/api/queries'

/**
 * Advance one store order along the machine (accept → prepare → pick up →
 * deliver), or cancel to reject. The server validates the transition; an
 * illegal move toasts its message. The board refetches afterwards.
 */
export function useAdvanceStoreOrder() {
  const queryClient = useQueryClient()
  return useMutation({
    ...advanceStoreOrderMutation(),
    onSuccess: (order) => {
      toast.success(`Order #${order.number} → ${order.status.replaceAll('_', ' ')}`)
      void queryClient.invalidateQueries({
        queryKey: listMerchantOrdersQueryKey(),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError
          ? error.message
          : 'Could not update the order',
      )
    },
  })
}
