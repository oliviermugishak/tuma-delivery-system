import { useQuery } from '@tanstack/react-query'

import { ApiError } from '@/api/client'
import { getOwnStoreOptions } from '@/api/queries'

/**
 * One of the merchant's stores by id. 404 means "not one of yours" (or
 * deleted) — a terminal state for the detail page, never retried.
 */
export function useOwnStore(storeId: string) {
  return useQuery({
    ...getOwnStoreOptions({ path: { id: storeId } }),
    enabled: storeId !== '',
    retry: (failureCount, error) => {
      if (error instanceof ApiError && error.status === 404) return false
      return failureCount < 3
    },
  })
}
