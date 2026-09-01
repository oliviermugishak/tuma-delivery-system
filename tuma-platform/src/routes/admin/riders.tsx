import { createFileRoute } from '@tanstack/react-router'

import { RidersPage } from '@/features/admin/components/riders'

export const Route = createFileRoute('/admin/riders')({
  component: RidersPage,
})
