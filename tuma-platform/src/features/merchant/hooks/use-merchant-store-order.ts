import { useQuery } from '@tanstack/react-query'

import { ApiError } from '@/api/client'
import { getMerchantStoreOrderOptions } from '@/api/queries'

/**
 * One store order's fulfillment sheet: items, delivery address, and the
 * customer contact. A 404 is final — the order is gone or not this
 * operator's; the dialog shows its not-found state instead of retrying.
 */
export function useMerchantStoreOrder(storeOrderId: string) {
  return useQuery({
    ...getMerchantStoreOrderOptions({ path: { id: storeOrderId } }),
    enabled: storeOrderId !== '',
    retry: (failureCount, error) => {
      if (error instanceof ApiError && error.status === 404) return false
      return failureCount < 3
    },
  })
}
