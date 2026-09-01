import { createFileRoute } from '@tanstack/react-router'

import { OrdersScreen } from '@/features/merchant/components/orders'

export const Route = createFileRoute('/merchant/orders/')({
  component: OrdersScreen,
})
