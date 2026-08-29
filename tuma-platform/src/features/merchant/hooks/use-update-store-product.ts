import { useMutation, useQueryClient } from '@tanstack/react-query'

import { ApiError } from '@/api/client'
import {
  listStoreProductsQueryKey,
  updateStoreProductMutation,
} from '@/api/queries'
import { toast } from 'sonner'

/**
 * Update a store_product's sell configuration — price, stock (a number
 * sets it, null switches back to untracked), availability, SKU. Absent
 * fields keep their values server-side, so callers send only what changed.
 * Quiet by design: availability flips are frequent, so no success toast.
 */
export function useUpdateStoreProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    ...updateStoreProductMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: listStoreProductsQueryKey(),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError
          ? error.message
          : 'Could not update the item',
      )
    },
  })
}
