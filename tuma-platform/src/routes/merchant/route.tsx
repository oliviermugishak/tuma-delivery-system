import { createFileRoute } from '@tanstack/react-router'
import {
  LayoutDashboard,
  Package,
  ReceiptText,
  ShoppingBasket,
  Store,
} from 'lucide-react'
import { requireWing } from '../../lib/session'
import { WingShell } from '../../components/wing-shell'

export const Route = createFileRoute('/merchant')({
  beforeLoad: requireWing('merchant'),
  component: MerchantWing,
})

// Settings is deliberately not a nav item — it pops out of the user badge
// at the bottom of the sidebar. Nav order follows the operating flow:
// Overview • Orders • Catalog • Assortment • Stores.
const nav = [
  { to: '/merchant/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/merchant/orders', label: 'Orders', icon: ReceiptText },
  { to: '/merchant/catalog', label: 'Catalog', icon: Package },
  { to: '/merchant/menu', label: 'Assortment', icon: ShoppingBasket },
  { to: '/merchant/store', label: 'Stores', icon: Store },
]

function MerchantWing() {
  return (
    <WingShell wing="Merchant" nav={nav} settingsTo="/merchant/settings" />
  )
}
