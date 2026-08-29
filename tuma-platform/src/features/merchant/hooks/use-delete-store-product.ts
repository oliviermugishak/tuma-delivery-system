import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  deleteStoreProductMutation,
  listStoreProductsQueryKey,
} from '@/api/queries'

/**
 * Detach one product from one store. The catalog identity stays — it can
 * come back (differently priced) any time. The UI confirms first.
 */
export function useDeleteStoreProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    ...deleteStoreProductMutation(),
    onSuccess: () => {
      toast.success('Removed from the store')
      void queryClient.invalidateQueries({
        queryKey: listStoreProductsQueryKey(),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError
          ? error.message
          : 'Could not remove the item',
      )
    },
  })
}
