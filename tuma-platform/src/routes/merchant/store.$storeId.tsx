import { createFileRoute } from '@tanstack/react-router'

import { StoreDetailScreen } from '@/features/merchant/components/store-detail'

export const Route = createFileRoute('/merchant/store/$storeId')({
  component: StoreDetailScreen,
})
