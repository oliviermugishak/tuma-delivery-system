import { useQuery } from '@tanstack/react-query'

import { listMerchantsOptions } from '@/api/queries'

/** All merchant accounts, oldest first. Admin-only endpoint. */
export function useMerchants() {
  return useQuery(listMerchantsOptions())
}
