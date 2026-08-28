import { useQuery } from '@tanstack/react-query'

import { listOwnProductsOptions } from '@/api/queries'

/** The merchant's full menu (available and unavailable), oldest first. */
export function useOwnProducts() {
  return useQuery(listOwnProductsOptions())
}
