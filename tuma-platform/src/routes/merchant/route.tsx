import { useMemo } from 'react'
import { Outlet, createFileRoute } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import {
  listMerchantOrdersOptions,
  listOwnStoresOptions,
} from '@/api/queries'
import { AppShell, type NavGroup, type PaletteItem } from '@/components/ds'
import { useSession } from '@/hooks/use-session'
import { requireWing } from '@/lib/session'

export const Route = createFileRoute('/merchant')({
  beforeLoad: requireWing('merchant'),
  component: MerchantWing,
})

/**
 * Merchant nav (constitution Part 4): OPERATE / STORE / MONEY. The
 * Orders badge counts the operator's open stores' in-flight orders
 * (placed/accepted/preparing) — "to accept" lives on the board itself.
 */
const navGroups: NavGroup[] = [
  {
    title: 'Operate',
    items: [
      { to: '/merchant', label: 'Overview', icon: 'space_dashboard' },
      { to: '/merchant/orders', label: 'Orders', icon: 'receipt_long', badge: 0 },
    ],
  },
  {
    title: 'Store',
    items: [
      { to: '/merchant/catalog', label: 'Catalog', icon: 'nutrition' },
      { to: '/merchant/menu', label: 'Menu', icon: 'restaurant_menu' },
      { to: '/merchant/store', label: 'Stores', icon: 'storefront' },
    ],
  },
  {
    title: 'Money',
    items: [
      {
        to: '/merchant/earnings',
        label: 'Earnings & Payouts',
        icon: 'payments',
      },
      { to: '/merchant/reviews', label: 'Reviews', icon: 'reviews' },
    ],
  },
]

const staticPalette: PaletteItem[] = [
  { id: 'nav-overview', group: 'Go to', label: 'Overview', icon: 'space_dashboard', to: '/merchant' },
  { id: 'nav-orders', group: 'Go to', label: 'Orders', icon: 'receipt_long', to: '/merchant/orders' },
  { id: 'nav-catalog', group: 'Go to', label: 'Catalog', icon: 'nutrition', to: '/merchant/catalog' },
  { id: 'nav-menu', group: 'Go to', label: 'Menu', icon: 'restaurant_menu', to: '/merchant/menu' },
  { id: 'nav-stores', group: 'Go to', label: 'Stores', icon: 'storefront', to: '/merchant/store' },
  { id: 'nav-earnings', group: 'Go to', label: 'Earnings & Payouts', icon: 'payments', to: '/merchant/earnings' },
  { id: 'nav-reviews', group: 'Go to', label: 'Reviews', icon: 'reviews', to: '/merchant/reviews' },
]

function MerchantWing() {
  const { data: user } = useSession()
  // Palette searches the operator's live operational data: orders by
  // number, stores by name (V1 size fits the client; a server-side
  // search endpoint is in BACKEND-GAPS.md).
  const stores = useQuery({ ...listOwnStoresOptions(), staleTime: 60_000 })
  const orders = useQuery({
    ...listMerchantOrdersOptions({ query: { limit: 50, offset: 0 } }),
    refetchInterval: 15_000,
    staleTime: 10_000,
  })

  // Derived, never mutated: the Orders badge is the operator's in-flight
  // count; the module-level navGroups template stays untouched.
  const groups: NavGroup[] = useMemo(
    () =>
      navGroups.map((group, gi) =>
        gi === 0
          ? {
              ...group,
              items: group.items.map((item, ii) =>
                ii === 1
                  ? {
                      ...item,
                      badge:
                        orders.data?.filter(
                          (o) =>
                            o.status !== 'delivered' &&
                            o.status !== 'cancelled',
                        ).length || undefined,
                    }
                  : item,
              ),
            }
          : group,
      ),
    [orders.data],
  )

  const entityItems: PaletteItem[] = [
    ...(stores.data ?? []).map((s) => ({
      id: `store-${s.id}`,
      group: 'Stores',
      label: s.name,
      hint: s.category ?? undefined,
      icon: 'storefront',
      to: `/merchant/store/${s.id}`,
    })),
    ...(orders.data ?? []).map((o) => ({
      id: `order-${o.id}`,
      group: 'Orders',
      label: `Order #${o.number}`,
      hint: `${o.total.toLocaleString('en-US')} RWF`,
      icon: 'receipt_long',
      to: '/merchant/orders',
    })),
  ]

  return (
    <AppShell
      wing="Merchant"
      user={user!}
      groups={groups}
      paletteItems={[...staticPalette, ...entityItems]}
      settingsTo="/merchant/settings"
    >
      <Outlet />
    </AppShell>
  )
}
