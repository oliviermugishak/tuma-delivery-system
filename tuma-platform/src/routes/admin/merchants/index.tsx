import { createFileRoute } from '@tanstack/react-router'

import { MerchantsPanel } from '@/features/admin/components/merchants-panel'

export const Route = createFileRoute('/admin/merchants/')({
  component: MerchantsPanel,
})
