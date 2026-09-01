import { createFileRoute } from '@tanstack/react-router'

import { ProductNewPage } from '@/features/merchant/components/product-form'

export const Route = createFileRoute('/merchant/menu/new')({
  component: ProductNewPage,
})
