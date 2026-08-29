import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  deleteOwnStoreMutation,
  listOwnStoresQueryKey,
  listStoreProductsQueryKey,
} from '@/api/queries'

/**
 * Hard-delete one of the business's stores. Permanent: its assortment
 * follows via ON DELETE CASCADE, so both the store list and the
 * assortment are refetched. The UI confirms in an alert dialog before
 * this ever runs.
 */
export function useDeleteStore() {
  const queryClient = useQueryClient()
  return useMutation({
    ...deleteOwnStoreMutation(),
    onSuccess: () => {
      toast.success('Store deleted')
      void queryClient.invalidateQueries({ queryKey: listOwnStoresQueryKey() })
      void queryClient.invalidateQueries({
        queryKey: listStoreProductsQueryKey(),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not delete the store',
      )
    },
  })
}
