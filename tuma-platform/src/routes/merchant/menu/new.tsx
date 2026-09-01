import { createFileRoute } from '@tanstack/react-router'

import { ProductNewPage } from '@/features/merchant/components/product-form'

type MenuNewSearch = {
  /** Preselect the catalog product to attach (from Catalog's kebab). */
  product?: string
}

export const Route = createFileRoute('/merchant/menu/new')({
  validateSearch: (search: Record<string, unknown>): MenuNewSearch => ({
    product:
      typeof search.product === 'string' ? search.product : undefined,
  }),
  component: ProductNewPage,
})
