import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  listProductImagesQueryKey,
  listProductsQueryKey,
  listStoreProductsQueryKey,
  uploadProductImageMutation,
} from '@/api/queries'

/**
 * Append one image to a catalog product's gallery. Refreshes the gallery
 * itself, the catalog list (its cover may change), and the assortment
 * (its cards show the same imagery).
 */
export function useUploadProductImage() {
  const queryClient = useQueryClient()
  return useMutation({
    ...uploadProductImageMutation(),
    onSuccess: (_image, variables) => {
      toast.success('Image added')
      void queryClient.invalidateQueries({
        queryKey: listProductImagesQueryKey({ path: { id: variables.path.id } }),
      })
      void queryClient.invalidateQueries({ queryKey: listProductsQueryKey() })
      void queryClient.invalidateQueries({ queryKey: listStoreProductsQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not upload the image',
      )
    },
  })
}
