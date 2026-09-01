import { createFileRoute } from '@tanstack/react-router'

import { DisputeResolve } from '@/features/admin/components/dispute-resolve'

export const Route = createFileRoute('/admin/disputes/$disputeId')({
  component: DisputeResolve,
})
