import { createFileRoute } from '@tanstack/react-router'
import { LayoutDashboard, Store, Users } from 'lucide-react'
import { requireRole } from '../../lib/session'
import { WingShell } from '../../components/wing-shell'

export const Route = createFileRoute('/admin')({
  beforeLoad: requireRole('admin'),
  component: AdminWing,
})

// Settings is deliberately not a nav item — it pops out of the user badge
// at the bottom of the sidebar.
const nav = [
  { to: '/admin/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/admin/merchants', label: 'Merchants', icon: Store },
  { to: '/admin/customers', label: 'Customers', icon: Users },
]

function AdminWing() {
  return <WingShell wing="Admin" nav={nav} settingsTo="/admin/settings" />
}
