import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  listProductImagesQueryKey,
  listProductsQueryKey,
  listStoreProductsQueryKey,
  setProductCoverMutation,
} from '@/api/queries'

/**
 * Make one gallery image the product's cover (the lowest position). The
 * catalog and assortment cards follow immediately via the cache refresh.
 */
export function useSetProductCover() {
  const queryClient = useQueryClient()
  return useMutation({
    ...setProductCoverMutation(),
    onSuccess: (_result, variables) => {
      toast.success('Cover updated')
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
        error instanceof ApiError ? error.message : 'Could not set the cover',
      )
    },
  })
}
