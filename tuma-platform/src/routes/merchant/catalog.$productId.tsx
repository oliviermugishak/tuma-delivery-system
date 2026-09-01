import { createFileRoute } from '@tanstack/react-router'

import { CatalogEditPage } from '@/features/merchant/components/catalog'

export const Route = createFileRoute('/merchant/catalog/$productId')({
  component: CatalogEditPage,
})
