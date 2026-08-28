import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  deleteOwnStoreMutation,
  listOwnProductsQueryKey,
  listOwnStoresQueryKey,
} from '@/api/queries'

/**
 * Hard-delete one of the merchant's own stores. Permanent: the store's
 * products follow via ON DELETE CASCADE, so both the store list and the
 * menu are refetched. The UI confirms in an alert dialog before this ever
 * runs.
 */
export function useDeleteStore() {
  const queryClient = useQueryClient()
  return useMutation({
    ...deleteOwnStoreMutation(),
    onSuccess: () => {
      toast.success('Store deleted')
      void queryClient.invalidateQueries({ queryKey: listOwnStoresQueryKey() })
      void queryClient.invalidateQueries({
        queryKey: listOwnProductsQueryKey(),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not delete the store',
      )
    },
  })
}
