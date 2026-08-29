import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  deleteProductMutation,
  listProductsQueryKey,
  listStoreProductsQueryKey,
} from '@/api/queries'

/**
 * Hard-delete a catalog product. Every store selling it loses it at once
 * (the server cascades the store_products away) — the UI confirms in an
 * alert dialog before this ever runs. Pausing per store stays the
 * everyday tool; catalog delete is for items that will never return.
 */
export function useDeleteCatalogProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    ...deleteProductMutation(),
    onSuccess: () => {
      toast.success('Product deleted everywhere')
      void queryClient.invalidateQueries({ queryKey: listProductsQueryKey() })
      void queryClient.invalidateQueries({
        queryKey: listStoreProductsQueryKey(),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError
          ? error.message
          : 'Could not delete the product',
      )
    },
  })
}
