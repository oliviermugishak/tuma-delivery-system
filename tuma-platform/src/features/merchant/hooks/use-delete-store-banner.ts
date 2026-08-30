import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  deleteStoreBannerMutation,
  getOwnStoreQueryKey,
  listOwnStoresQueryKey,
} from '@/api/queries'

/** Clear a store's banner. The object is retired server-side. */
export function useDeleteStoreBanner() {
  const queryClient = useQueryClient()
  return useMutation({
    ...deleteStoreBannerMutation(),
    onSuccess: (_result, variables) => {
      toast.success('Banner removed')
      void queryClient.invalidateQueries({ queryKey: listOwnStoresQueryKey() })
      void queryClient.invalidateQueries({
        queryKey: getOwnStoreQueryKey({ path: { id: variables.path.id } }),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not remove the banner',
      )
    },
  })
}
