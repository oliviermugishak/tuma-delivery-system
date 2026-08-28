import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { createProductMutation, listOwnProductsQueryKey } from '@/api/queries'

/**
 * Add a product to the menu. 404 means the merchant has no store yet —
 * unreachable from the UI (the menu card only renders once the store
 * exists), but the server message still toasts cleanly if it ever happens.
 */
export function useCreateProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    ...createProductMutation(),
    onSuccess: (product) => {
      toast.success(`"${product.name}" added to the menu`)
      void queryClient.invalidateQueries({
        queryKey: listOwnProductsQueryKey(),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not add the product',
      )
    },
  })
}
