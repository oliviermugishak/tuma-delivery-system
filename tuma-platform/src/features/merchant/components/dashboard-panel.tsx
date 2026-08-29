import { useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import {
  Building2,
  DoorOpen,
  Package,
  Plus,
  RefreshCw,
  Store,
  UtensilsCrossed,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'

import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { CreateStoreDialog } from '@/features/merchant/components/create-store-dialog'
import { StoreCard } from '@/features/merchant/components/store-card'
import { useStoreProducts } from '@/features/merchant/hooks/use-store-products'
import { useOwnStores } from '@/features/merchant/hooks/use-stores'

/**
 * Merchant dashboard on real state: counts derived from the merchant's own
 * stores and menu, their stores as cards, and a way into the menu. No fake
 * KPIs — order metrics land with the orders iteration.
 */
export function DashboardPanel() {
  const stores = useOwnStores()
  const assortment = useStoreProducts()
  const [createOpen, setCreateOpen] = useState(false)

  const stats = useMemo(() => {
    const storeList = stores.data ?? []
    const items = assortment.data ?? []
    return {
      stores: storeList.length,
      openStores: storeList.filter((store) => store.is_open).length,
      items: items.length,
      available: items.filter((item) => item.is_available).length,
    }
  }, [stores.data, assortment.data])

  const productCounts = useMemo(() => {
    const counts = new Map<string, number>()
    for (const item of assortment.data ?? []) {
      counts.set(item.store_id, (counts.get(item.store_id) ?? 0) + 1)
    }
    return counts
  }, [assortment.data])

  const isLoading = stores.isLoading || assortment.isLoading
  const isError = stores.isError || assortment.isError
  const retry = () => {
    void stores.refetch()
    void assortment.refetch()
  }

  return (
    <div>
      <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="font-heading text-2xl font-semibold tracking-tight">
            Dashboard
          </h2>
          <p className="mt-1.5 max-w-xl text-sm text-muted-foreground">
            Your business at a glance — stores, assortment and
            availability, live from the server.
          </p>
        </div>
        <Button
          variant="outline"
          render={(props) => <Link {...props} to="/merchant/menu" />}
        >
          <UtensilsCrossed data-icon="inline-start" />
          Manage assortment
        </Button>
      </div>

      {isLoading ? <DashboardSkeleton /> : null}

      {isError ? (
        <Card className="max-w-3xl">
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">Couldn&apos;t load your dashboard</p>
            <p className="text-sm text-muted-foreground">
              Something went wrong while fetching your stores and menu.
            </p>
            <Button variant="outline" size="sm" onClick={retry}>
              <RefreshCw data-icon="inline-start" />
              Try again
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {stores.isSuccess && assortment.isSuccess ? (
        <>
          <div className="grid gap-6 sm:grid-cols-2 xl:grid-cols-4">
            <StatCard label="Stores" value={stats.stores} icon={Building2} />
            <StatCard
              label="Open now"
              value={stats.openStores}
              icon={DoorOpen}
            />
            <StatCard label="Assortment" value={stats.items} icon={Package} />
            <StatCard
              label="Available"
              value={stats.available}
              icon={UtensilsCrossed}
            />
          </div>

          <section className="mt-10">
            <div className="mb-4 flex items-center justify-between gap-4">
              <h3 className="font-heading text-lg font-semibold">
                Your stores
              </h3>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setCreateOpen(true)}
              >
                <Plus data-icon="inline-start" />
                Create store
              </Button>
            </div>

            {stores.data.length === 0 ? (
              <Card>
                <CardContent className="flex flex-col items-center gap-4 py-20 text-center">
                  <div className="flex size-14 items-center justify-center rounded-2xl bg-primary/10">
                    <Store className="size-7 text-primary" aria-hidden />
                  </div>
                  <div className="grid gap-1">
                    <p className="font-heading text-lg font-semibold">
                      No stores yet
                    </p>
                    <p className="mx-auto max-w-sm text-sm text-muted-foreground">
                      Create your first store — then attach catalog products
                      to its assortment and open it for orders.
                    </p>
                  </div>
                  <Button onClick={() => setCreateOpen(true)}>
                    <Plus data-icon="inline-start" />
                    Create your first store
                  </Button>
                </CardContent>
              </Card>
            ) : (
              <div className="grid gap-6 lg:grid-cols-2">
                {stores.data.map((store) => (
                  <StoreCard
                    key={store.id}
                    store={store}
                    productCount={productCounts.get(store.id) ?? 0}
                  />
                ))}
              </div>
            )}
          </section>

          <Card className="mt-10 max-w-3xl">
            <CardHeader className="flex flex-row items-center justify-between space-y-0">
              <div className="grid gap-1.5">
                <CardTitle className="text-base">Orders</CardTitle>
                <CardDescription>
                  Incoming store orders live on the Orders page — accept,
                  prepare, and hand them off there.
                </CardDescription>
              </div>
              <Button
                variant="outline"
                render={(props) => <Link {...props} to="/merchant/orders" />}
              >
                Open Orders
              </Button>
            </CardHeader>
          </Card>
        </>
      ) : null}

      <CreateStoreDialog open={createOpen} onOpenChange={setCreateOpen} />
    </div>
  )
}

function StatCard({
  label,
  value,
  icon: Icon,
}: {
  label: string
  value: number
  icon: LucideIcon
}) {
  return (
    <Card>
      <CardContent className="flex items-start justify-between p-6">
        <div>
          <p className="text-xs font-medium tracking-wider text-muted-foreground uppercase">
            {label}
          </p>
          <p className="mt-2 font-heading text-3xl font-semibold tracking-tight">
            {value}
          </p>
        </div>
        <div className="flex size-10 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Icon className="size-5" aria-hidden />
        </div>
      </CardContent>
    </Card>
  )
}

function DashboardSkeleton() {
  return (
    <div className="space-y-6">
      <div className="grid gap-6 sm:grid-cols-2 xl:grid-cols-4">
        {Array.from({ length: 4 }).map((_, i) => (
          <Skeleton key={i} className="h-28 rounded-xl" />
        ))}
      </div>
      <div className="grid gap-6 lg:grid-cols-2">
        <Skeleton className="h-52 rounded-xl" />
        <Skeleton className="h-52 rounded-xl" />
      </div>
    </div>
  )
}
