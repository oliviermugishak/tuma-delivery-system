import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  listProductImagesQueryKey,
  listProductsQueryKey,
  listStoreProductsQueryKey,
  deleteProductImageMutation,
} from '@/api/queries'

/**
 * Remove one gallery image. The server deletes the object as well as the
 * row; if the removed image was the cover, the next-lowest takes over.
 */
export function useDeleteProductImage() {
  const queryClient = useQueryClient()
  return useMutation({
    ...deleteProductImageMutation(),
    onSuccess: (_result, variables) => {
      toast.success('Image removed')
      void queryClient.invalidateQueries({
        queryKey: listProductImagesQueryKey({
          path: { id: variables.path.product_id },
        }),
      })
      void queryClient.invalidateQueries({ queryKey: listProductsQueryKey() })
      void queryClient.invalidateQueries({ queryKey: listStoreProductsQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not remove the image',
      )
    },
  })
}
