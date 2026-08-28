import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { listOwnProductsQueryKey, updateProductMutation } from '@/api/queries'

/**
 * Update one of the merchant's products — the edit dialog and the per-row
 * availability switch both come through here. Absent fields keep their
 * current values server-side, so callers send only what changed.
 */
export function useUpdateProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    ...updateProductMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: listOwnProductsQueryKey(),
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
