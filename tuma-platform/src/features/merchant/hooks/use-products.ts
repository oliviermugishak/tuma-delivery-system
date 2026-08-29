import { useQuery } from '@tanstack/react-query'

import { listProductsOptions } from '@/api/queries'

/**
 * The business's catalog — the product identities this merchant defines
 * once and can attach to any store. Oldest first.
 */
export function useCatalogProducts() {
  return useQuery(listProductsOptions())
}
