import { createFileRoute } from '@tanstack/react-router'

import { AdminOverview } from '@/features/admin/components/admin-overview'

export const Route = createFileRoute('/admin/')({
  component: AdminOverview,
})
