import { createFileRoute } from '@tanstack/react-router'

import { RiderNewPage } from '@/features/admin/components/rider-form'

export const Route = createFileRoute('/admin/riders/new')({
  component: RiderNewPage,
})
