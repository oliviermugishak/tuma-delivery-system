import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import {
  getOwnStoreQueryKey,
  listOwnStoresQueryKey,
  uploadStoreBannerMutation,
} from '@/api/queries'

/**
 * Upload (or replace) one store's banner. The server validates, resizes,
 * and stores the bytes; the response's image_url serves the new banner
 * from the configured storage base. Replaces retire the old object
 * server-side.
 */
export function useUploadStoreBanner() {
  const queryClient = useQueryClient()
  return useMutation({
    ...uploadStoreBannerMutation(),
    onSuccess: (_store, variables) => {
      toast.success('Banner updated')
      void queryClient.invalidateQueries({ queryKey: listOwnStoresQueryKey() })
      void queryClient.invalidateQueries({
        queryKey: getOwnStoreQueryKey({ path: { id: variables.path.id } }),
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not upload the banner',
      )
    },
  })
}
