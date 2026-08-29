import { createFileRoute } from '@tanstack/react-router'

import { OrdersPanel } from '@/features/merchant/components/orders-panel'

export const Route = createFileRoute('/merchant/orders')({
  component: OrdersPanel,
})
