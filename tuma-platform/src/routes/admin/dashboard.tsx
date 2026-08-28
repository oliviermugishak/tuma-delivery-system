import { createFileRoute } from '@tanstack/react-router'
import { DashboardPanel } from '../../features/admin/components/dashboard-panel'

export const Route = createFileRoute('/admin/dashboard')({
  component: AdminDashboard,
})

function AdminDashboard() {
  return <DashboardPanel />
}
