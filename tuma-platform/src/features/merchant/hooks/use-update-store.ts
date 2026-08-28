import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  getOwnStoreQueryKey,
  listOwnStoresQueryKey,
  updateOwnStoreMutation,
} from '@/api/queries'

/**
 * Update one of the merchant's stores — the detail page's edit form and the
 * open-for-orders switch both come through here. The server's PATCH keeps
 * absent fields as they were, so callers send only what changed. Both the
 * list and the single-store caches are refreshed.
 */
export function useUpdateStore() {
  const queryClient = useQueryClient()
  return useMutation({
    ...updateOwnStoreMutation(),
    onSuccess: (_store, variables) => {
      toast.success('Store updated')
      void queryClient.invalidateQueries({ queryKey: listOwnStoresQueryKey() })
      void queryClient.invalidateQueries({
        queryKey: getOwnStoreQueryKey({ path: { id: variables.path.id } }),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not update the store',
      )
    },
  })
}
