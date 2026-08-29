import { useQuery } from '@tanstack/react-query'

import { listMerchantOrdersOptions } from '@/api/queries'

/**
 * Incoming store orders across every store the operator can reach,
 * newest first. This is the operating list: accept, prepare, hand off.
 */
export function useMerchantOrders(limit = 50, offset = 0) {
  return useQuery({
    ...listMerchantOrdersOptions({ query: { limit, offset } }),
    // The board is live operational state — refetch while mounted.
    refetchInterval: 10_000,
  })
}
