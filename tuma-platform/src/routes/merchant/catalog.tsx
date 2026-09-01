import { createFileRoute } from '@tanstack/react-router'

import { CatalogScreen } from '@/features/merchant/components/catalog'

export const Route = createFileRoute('/merchant/catalog')({
  component: CatalogScreen,
})
