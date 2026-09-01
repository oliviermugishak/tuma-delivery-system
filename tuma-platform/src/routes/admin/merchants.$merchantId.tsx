import { createFileRoute } from '@tanstack/react-router'

import { MerchantDetailPage } from '@/features/admin/components/merchant-detail'

export const Route = createFileRoute('/admin/merchants/$merchantId')({
  component: MerchantDetailPage,
})
