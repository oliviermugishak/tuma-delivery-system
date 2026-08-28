import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  deleteProductMutation,
  listOwnProductsQueryKey,
} from '@/api/queries'

/**
 * Hard-delete one of the merchant's own products. Permanent — the UI
 * confirms in an alert dialog before this ever runs. Pausing (the
 * availability switch) stays the everyday tool; delete is for items that
 * will never return. Refetches the menu afterwards.
 */
export function useDeleteProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    ...deleteProductMutation(),
    onSuccess: () => {
      toast.success('Product deleted')
      void queryClient.invalidateQueries({
        queryKey: listOwnProductsQueryKey(),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not delete the product',
      )
    },
  })
}
