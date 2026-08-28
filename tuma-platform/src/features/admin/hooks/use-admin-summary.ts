import { useQuery } from '@tanstack/react-query'

import { summaryOptions } from '@/api/queries'

/** Platform-wide counts for the admin dashboard. Admin-only endpoint. */
export function useAdminSummary() {
  return useQuery(summaryOptions())
}
