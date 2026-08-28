import { createFileRoute } from '@tanstack/react-router'

import { StoreDetailPanel } from '@/features/merchant/components/store-detail-panel'

export const Route = createFileRoute('/merchant/store/$storeId')({
  component: StoreDetail,
})

function StoreDetail() {
  const { storeId } = Route.useParams()
  return <StoreDetailPanel storeId={storeId} />
}
