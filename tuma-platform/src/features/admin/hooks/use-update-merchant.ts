import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { listMerchantsQueryKey, updateMerchantMutation } from '@/api/queries'

/**
 * Edit a merchant business (admin-only): name, business contact, and the
 * status all ride the same PATCH — suspending is an edit like any other.
 * Provided fields overwrite, an empty string clears an optional contact,
 * absent fields keep their value. Toast on both outcomes and refetch the
 * list + whichever detail page is open.
 */
export function useUpdateMerchant() {
  const queryClient = useQueryClient()
  return useMutation({
    ...updateMerchantMutation(),
    onSuccess: (merchant, variables) => {
      const label = merchant.name
      if (variables.body?.status !== undefined) {
        toast.success(
          merchant.status === 'active'
            ? `"${label}" is active again`
            : `"${label}" is suspended`,
        )
      } else {
        toast.success(`"${label}" updated`)
      }
      void queryClient.invalidateQueries({ queryKey: listMerchantsQueryKey() })
      // The detail page holds its own getMerchant query — refresh whichever
      // one matches so it follows the server too.
      void queryClient.invalidateQueries({
        predicate: (query) =>
          (query.queryKey[0] as { _id?: string } | undefined)?._id ===
          'getMerchant',
      })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not update the merchant',
      )
    },
  })
}
