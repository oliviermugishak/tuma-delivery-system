import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { createMerchantMutation, listMerchantsQueryKey } from '@/api/queries'

/**
 * Provision a merchant business (admin-only): the business row, the owner's
 * account, and the owner membership — one call. Merchants never self-signup.
 * On success: toast and refetch the list. On failure: surface the server's
 * own message (e.g. the 409 "email already taken") so the admin can fix the
 * input and retry.
 */
export function useCreateMerchant() {
  const queryClient = useQueryClient()
  return useMutation({
    ...createMerchantMutation(),
    onSuccess: (created) => {
      toast.success(
        `Business "${created.name}" created — owner ${created.owner_email}`,
      )
      void queryClient.invalidateQueries({ queryKey: listMerchantsQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not create the merchant',
      )
    },
  })
}
