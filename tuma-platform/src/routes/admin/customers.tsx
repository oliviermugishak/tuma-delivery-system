import { createFileRoute } from '@tanstack/react-router'

import { CustomersPanel } from '@/features/admin/components/customers-panel'

export const Route = createFileRoute('/admin/customers')({
  component: CustomersPanel,
})
