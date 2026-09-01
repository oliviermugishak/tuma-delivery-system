import { createFileRoute } from '@tanstack/react-router'

import { DisputesQueue } from '@/features/admin/components/disputes'

export const Route = createFileRoute('/admin/disputes')({
  component: DisputesQueue,
})
