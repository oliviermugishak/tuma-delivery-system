import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { createOwnStoreMutation, listOwnStoresQueryKey } from '@/api/queries'

/**
 * Create one of the merchant's stores — merchants can have several, so this
 * stays available from the stores list at any time. New stores start
 * closed; opening is a deliberate act on the store's detail page.
 */
export function useCreateStore() {
  const queryClient = useQueryClient()
  return useMutation({
    ...createOwnStoreMutation(),
    onSuccess: (store) => {
      toast.success(`Store "${store.name}" created`)
      void queryClient.invalidateQueries({ queryKey: listOwnStoresQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not create the store',
      )
    },
  })
}
