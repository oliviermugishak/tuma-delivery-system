import { useQuery } from '@tanstack/react-query'

import { ApiError } from '@/api/client'
import { getMerchantOptions } from '@/api/queries'

/**
 * One merchant with their stores and menu, for the admin detail page. A 404
 * is final — the page shows its not-found state instead of retrying.
 */
export function useAdminMerchant(id: string) {
  return useQuery({
    ...getMerchantOptions({ path: { id } }),
    retry: (failureCount, error) => {
      if (error instanceof ApiError && error.status === 404) return false
      return failureCount < 3
    },
  })
}
