import { createFileRoute } from '@tanstack/react-router'

import { MerchantsPage } from '@/features/admin/components/merchants'

export const Route = createFileRoute('/admin/merchants/')({
  component: MerchantsPage,
})
