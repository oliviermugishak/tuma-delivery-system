import { createFileRoute } from '@tanstack/react-router'

import { ProductEditPage } from '@/features/merchant/components/product-form'

export const Route = createFileRoute('/merchant/menu/$productId')({
  component: ProductEditPage,
})
