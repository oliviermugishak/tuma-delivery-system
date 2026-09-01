import { createFileRoute } from '@tanstack/react-router'

import { FeesPage } from '@/features/admin/components/fees'

export const Route = createFileRoute('/admin/fees')({
  component: FeesPage,
})
