import { useQuery } from '@tanstack/react-query'

import { listRidersOptions } from '@/api/queries'

/** All riders, oldest first. Admin-only endpoint. */
export function useRiders() {
  return useQuery(listRidersOptions())
}
