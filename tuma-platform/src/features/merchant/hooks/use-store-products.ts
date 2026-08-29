import { useQuery } from '@tanstack/react-query'

import { listStoreProductsOptions } from '@/api/queries'

/**
 * The assortment across every store the operator can reach: one row per
 * store_product with its catalog identity, per-store price, stock and
 * availability. Oldest first.
 */
export function useStoreProducts() {
  return useQuery(listStoreProductsOptions())
}
