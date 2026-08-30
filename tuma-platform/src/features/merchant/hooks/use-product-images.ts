import { useQuery } from '@tanstack/react-query'

import { listProductImagesOptions } from '@/api/queries'

/**
 * One product's gallery, cover first. Used by the catalog edit dialog's
 * gallery editor.
 */
export function useProductImages(productId: string) {
  return useQuery(listProductImagesOptions({ path: { id: productId } }))
}
