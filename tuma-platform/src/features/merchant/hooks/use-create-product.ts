import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { createProductMutation, listProductsQueryKey } from '@/api/queries'

/**
 * Add a catalog product — the merchant-level identity a store then sells.
 * Attaching it to a store (with a price and stock) is a separate step on
 * the assortment side. On failure the server's message toasts (e.g.
 * owner-only guard) so the operator knows why.
 */
export function useCreateCatalogProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    ...createProductMutation(),
    onSuccess: (product) => {
      toast.success(`"${product.name}" added to the catalog`)
      void queryClient.invalidateQueries({ queryKey: listProductsQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError
          ? error.message
          : 'Could not add the product',
      )
    },
  })
}
