import { createFileRoute } from '@tanstack/react-router'

import { CustomersPage } from '@/features/admin/components/customers'

export const Route = createFileRoute('/admin/customers/')({
  component: CustomersPage,
})
