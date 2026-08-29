import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { listProductsQueryKey, updateProductMutation } from '@/api/queries'

/**
 * Edit a catalog product — name, description, image. Prices do NOT live
 * here: they are per store (store_products). Absent fields keep their
 * values server-side, so callers send only what changed.
 */
export function useUpdateCatalogProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    ...updateProductMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: listProductsQueryKey() })
      // Assortment views render the catalog name — refresh them too.
      void queryClient.invalidateQueries({
        queryKey: listStoreProductsQueryKey(),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError
          ? error.message
          : 'Could not update the product',
      )
    },
  })
}

import { listStoreProductsQueryKey } from '@/api/queries'
