import { createFileRoute } from '@tanstack/react-router'

import { CatalogPanel } from '@/features/merchant/components/catalog-panel'

export const Route = createFileRoute('/merchant/catalog')({
  component: CatalogPanel,
})
