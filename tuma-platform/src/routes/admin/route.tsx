import { Outlet, createFileRoute } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'

import {
  listCustomersOptions,
  listMerchantsOptions,
  listRidersOptions,
} from '@/api/queries'
import { AppShell, type NavGroup, type PaletteItem } from '@/components/ds'
import { useSession } from '@/hooks/use-session'
import { requireWing } from '@/lib/session'

export const Route = createFileRoute('/admin')({
  beforeLoad: requireWing('admin'),
  component: AdminWing,
})

/**
 * Admin nav (constitution Part 4): OPERATE / NETWORK / PLATFORM. Founders'
 * call (2026-09-01): admins never see orders — merchants own the order
 * lifecycle; the admin wing manages the network (merchants, riders,
 * customers) and the platform. Settings lives under the avatar.
 */
const navGroups: NavGroup[] = [
  {
    title: 'Operate',
    items: [
      { to: '/admin', label: 'Overview', icon: 'space_dashboard' },
      {
        to: '/admin/disputes',
        label: 'Disputes & Refunds',
        icon: 'gavel',
      },
    ],
  },
  {
    title: 'Network',
    items: [
      { to: '/admin/merchants', label: 'Merchants', icon: 'storefront' },
      { to: '/admin/riders', label: 'Riders', icon: 'two_wheeler' },
      { to: '/admin/customers', label: 'Customers', icon: 'groups' },
    ],
  },
  {
    title: 'Platform',
    items: [
      { to: '/admin/fees', label: 'Fees & Categories', icon: 'sell' },
      { to: '/admin/audit', label: 'Audit log', icon: 'history' },
    ],
  },
]

const staticPalette: PaletteItem[] = [
  { id: 'nav-overview', group: 'Go to', label: 'Overview', icon: 'space_dashboard', to: '/admin' },
  { id: 'nav-disputes', group: 'Go to', label: 'Disputes & Refunds', icon: 'gavel', to: '/admin/disputes' },
  { id: 'nav-merchants', group: 'Go to', label: 'Merchants', icon: 'storefront', to: '/admin/merchants' },
  { id: 'nav-riders', group: 'Go to', label: 'Riders', icon: 'two_wheeler', to: '/admin/riders' },
  { id: 'nav-customers', group: 'Go to', label: 'Customers', icon: 'groups', to: '/admin/customers' },
  { id: 'nav-fees', group: 'Go to', label: 'Fees & Categories', icon: 'sell', to: '/admin/fees' },
  { id: 'nav-audit', group: 'Go to', label: 'Audit log', icon: 'history', to: '/admin/audit' },
]

function AdminWing() {
  const groups = navGroups
  const { data: user } = useSession()
  // The palette reads the wing's own lists — small in V1, cached, and
  // shared with the pages that render them. No dedicated search endpoint
  // exists yet (BACKEND-GAPS.md): ID/phone search lands with it.
  const merchants = useQuery({ ...listMerchantsOptions(), staleTime: 60_000 })
  const riders = useQuery({ ...listRidersOptions(), staleTime: 60_000 })
  const customers = useQuery({ ...listCustomersOptions(), staleTime: 60_000 })

  const entityItems: PaletteItem[] = [
    ...(merchants.data ?? []).map((m) => ({
      id: `merchant-${m.id}`,
      group: 'Merchants',
      label: m.name,
      hint: m.business_email ?? undefined,
      icon: 'storefront',
      to: `/admin/merchants/${m.id}`,
    })),
    ...(riders.data ?? []).map((r) => ({
      id: `rider-${r.id}`,
      group: 'Riders',
      label: r.name,
      hint: `Rider #${r.rider_number}`,
      icon: 'two_wheeler',
      to: '/admin/riders',
    })),
    ...(customers.data ?? []).map((c) => ({
      id: `customer-${c.user_id}`,
      group: 'Customers',
      label: c.name ?? 'Unnamed customer',
      hint: c.phone ?? undefined,
      icon: 'person',
      to: '/admin/customers',
    })),
  ]

  return (
    <AppShell
      wing="Admin"
      user={user!}
      groups={groups}
      paletteItems={[...staticPalette, ...entityItems]}
      settingsTo="/admin/settings"
    >
      <Outlet />
    </AppShell>
  )
}
