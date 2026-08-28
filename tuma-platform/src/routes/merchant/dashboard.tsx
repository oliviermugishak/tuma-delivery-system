import { createFileRoute } from '@tanstack/react-router'

import { DashboardPanel } from '@/features/merchant/components/dashboard-panel'

export const Route = createFileRoute('/merchant/dashboard')({
  component: DashboardPanel,
})
