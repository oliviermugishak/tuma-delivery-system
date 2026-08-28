import { createFileRoute } from '@tanstack/react-router'
import { LayoutDashboard, Store, UtensilsCrossed } from 'lucide-react'
import { requireRole } from '../../lib/session'
import { WingShell } from '../../components/wing-shell'

export const Route = createFileRoute('/merchant')({
  beforeLoad: requireRole('merchant'),
  component: MerchantWing,
})

// Settings is deliberately not a nav item — it pops out of the user badge
// at the bottom of the sidebar. Nav order follows the blueprint's merchant
// system: Overview • Menu • Store.
const nav = [
  { to: '/merchant/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/merchant/menu', label: 'Menu', icon: UtensilsCrossed },
  { to: '/merchant/store', label: 'Stores', icon: Store },
]

function MerchantWing() {
  return (
    <WingShell wing="Merchant" nav={nav} settingsTo="/merchant/settings" />
  )
}
