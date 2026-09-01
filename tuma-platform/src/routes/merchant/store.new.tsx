import { createFileRoute } from '@tanstack/react-router'

import { StoreNewPage } from '@/features/merchant/components/store-form'

export const Route = createFileRoute('/merchant/store/new')({
  component: StoreNewPage,
})
