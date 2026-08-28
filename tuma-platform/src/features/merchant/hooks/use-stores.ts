import { useQuery } from '@tanstack/react-query'

import { listOwnStoresOptions } from '@/api/queries'

/** All of the signed-in merchant's stores, oldest first (empty = none yet). */
export function useOwnStores() {
  return useQuery(listOwnStoresOptions())
}
