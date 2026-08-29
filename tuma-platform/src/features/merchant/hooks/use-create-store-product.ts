import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  createStoreProductMutation,
  listStoreProductsQueryKey,
} from '@/api/queries'

/**
 * Attach a catalog product to a store: its price there, its stock (or
 * none — untracked), and its availability. A 409 means the store already
 * sells it — patch that store_product instead.
 */
export function useCreateStoreProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    ...createStoreProductMutation(),
    onSuccess: (item) => {
      toast.success(
        `"${item.product_name}" is now on ${item.store_name}'s menu`,
      )
      void queryClient.invalidateQueries({
        queryKey: listStoreProductsQueryKey(),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not attach the product',
      )
    },
  })
}
