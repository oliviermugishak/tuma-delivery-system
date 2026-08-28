import { useQuery } from '@tanstack/react-query'

import { listCustomersOptions } from '@/api/queries'

/** All customer accounts, oldest first. Admin-only endpoint. */
export function useCustomers() {
  return useQuery(listCustomersOptions())
}
