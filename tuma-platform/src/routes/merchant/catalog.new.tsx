import { createFileRoute } from '@tanstack/react-router'

import { CatalogNewPage } from '@/features/merchant/components/catalog'

export const Route = createFileRoute('/merchant/catalog/new')({
  component: CatalogNewPage,
})
