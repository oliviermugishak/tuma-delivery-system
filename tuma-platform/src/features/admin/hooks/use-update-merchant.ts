import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { listMerchantsQueryKey, updateMerchantMutation } from '@/api/queries'

/**
 * Edit a merchant (admin-only): name, email, and the active flag all ride
 * the same PATCH — toggling is an edit like any other. Provided fields
 * overwrite, an empty name clears it, absent fields keep their value. A
 * taken email surfaces the server's 409 message. Toast on both outcomes
 * and refetch the list + whichever detail page is open.
 */
export function useUpdateMerchant() {
  const queryClient = useQueryClient()
  return useMutation({
    ...updateMerchantMutation(),
    onSuccess: (merchant, variables) => {
      const label = merchant.name || merchant.email || 'Merchant'
      if (variables.body?.is_active !== undefined) {
        toast.success(
          merchant.is_active
            ? `"${label}" is now active`
            : `"${label}" is now deactivated`,
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
