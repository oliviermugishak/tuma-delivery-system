import { createFileRoute } from '@tanstack/react-router'

import { MerchantEditPage } from '@/features/admin/components/merchant-form'

export const Route = createFileRoute('/admin/merchants/$merchantId/edit')({
  component: MerchantEditPage,
})
