import { createFileRoute } from '@tanstack/react-router'

import { EarningsScreen } from '@/features/merchant/components/earnings'

export const Route = createFileRoute('/merchant/earnings/')({
  component: EarningsScreen,
})
