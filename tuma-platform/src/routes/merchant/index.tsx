import { createFileRoute } from '@tanstack/react-router'

import { MerchantOverview } from '@/features/merchant/components/merchant-overview'

export const Route = createFileRoute('/merchant/')({
  component: MerchantOverview,
})
