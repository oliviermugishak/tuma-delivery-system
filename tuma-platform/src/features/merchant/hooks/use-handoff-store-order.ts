import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  getMerchantStoreOrderQueryKey,
  handoffStoreOrderMutation,
  listMerchantOrdersQueryKey,
} from '@/api/queries'

/**
 * The real handoff: the operator types the rider's unique number and the
 * server assigns that rider, stamps the handoff, and caches the delivery
 * route — the event that moves the order to `picked_up`. A wrong number
 * (or an inactive rider) is a 404 with the server's message; the dialog
 * stays open so the number can be corrected. Re-assignment while the
 * delivery is out reuses this same action.
 */
export function useHandoffStoreOrder() {
  const queryClient = useQueryClient()
  return useMutation({
    ...handoffStoreOrderMutation(),
    onSuccess: (order) => {
      toast.success(
        `Order #${order.number} handed to rider — out for delivery`,
      )
      void queryClient.invalidateQueries({
        queryKey: listMerchantOrdersQueryKey(),
      })
      void queryClient.invalidateQueries({
        queryKey: getMerchantStoreOrderQueryKey({
          path: { id: order.id },
        }),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError
          ? error.message
          : 'Could not hand the order to the rider',
      )
    },
  })
}
