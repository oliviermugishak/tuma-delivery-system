import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { deleteMerchantMutation, listMerchantsQueryKey } from '@/api/queries'

/**
 * Hard-delete a merchant (admin-only). Permanent: their stores and
 * products follow via ON DELETE CASCADE. The UI confirms in an alert
 * dialog before this ever runs. Refetches the list afterwards.
 */
export function useDeleteMerchant() {
  const queryClient = useQueryClient()
  return useMutation({
    ...deleteMerchantMutation(),
    onSuccess: (_data, variables) => {
      toast.success('Merchant deleted')
      void queryClient.invalidateQueries({ queryKey: listMerchantsQueryKey() })
      // If the deleted merchant's detail page is in the cache, mark it
      // stale so going back shows the honest 404 state instead of stale
      // data.
      void queryClient.invalidateQueries({
        predicate: (query) => {
          const key = query.queryKey[0] as
            | { _id?: string; path?: { id?: string } }
            | undefined
          return key?._id === 'getMerchant' && key.path?.id === variables.path.id
        },
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not delete the merchant',
      )
    },
  })
}
