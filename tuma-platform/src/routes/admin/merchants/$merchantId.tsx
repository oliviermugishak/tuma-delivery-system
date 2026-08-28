import { createFileRoute } from '@tanstack/react-router'

import { MerchantDetailPanel } from '@/features/admin/components/merchant-detail-panel'

export const Route = createFileRoute('/admin/merchants/$merchantId')({
  component: MerchantDetail,
})

function MerchantDetail() {
  const { merchantId } = Route.useParams()
  return <MerchantDetailPanel merchantId={merchantId} />
}
