import { createFileRoute } from '@tanstack/react-router'

import { AuditPage } from '@/features/admin/components/audit'

export const Route = createFileRoute('/admin/audit')({
  component: AuditPage,
})
