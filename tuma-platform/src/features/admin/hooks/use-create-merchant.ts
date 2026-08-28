import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { createMerchantMutation, listMerchantsQueryKey } from '@/api/queries'

/**
 * Create a merchant account (admin-only). Merchants never self-signup — this
 * endpoint is the only way one comes into existence. On success: toast and
 * refetch the list. On failure: surface the server's own message (e.g. the
 * 409 "email already taken") so the admin can fix the input and retry.
 */
export function useCreateMerchant() {
  const queryClient = useQueryClient()
  return useMutation({
    ...createMerchantMutation(),
    onSuccess: (merchant) => {
      toast.success(`Merchant "${merchant.name || merchant.email}" created`)
      void queryClient.invalidateQueries({ queryKey: listMerchantsQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not create the merchant',
      )
    },
  })
}
