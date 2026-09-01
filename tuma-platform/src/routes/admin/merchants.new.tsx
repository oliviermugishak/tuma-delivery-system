import { createFileRoute } from '@tanstack/react-router'

import { MerchantNewPage } from '@/features/admin/components/merchant-form'

export const Route = createFileRoute('/admin/merchants/new')({
  component: MerchantNewPage,
})
