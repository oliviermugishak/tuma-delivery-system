import { useMemo, useState } from 'react'
import { Plus, RefreshCw, Store } from 'lucide-react'

import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { CreateStoreDialog } from '@/features/merchant/components/create-store-dialog'
import { StoreCard } from '@/features/merchant/components/store-card'
import { useStoreProducts } from '@/features/merchant/hooks/use-store-products'
import { useOwnStores } from '@/features/merchant/hooks/use-stores'

/**
 * The merchant's portfolio: every store they own as a card, clickable
 * through to its detail page. Merchants can have several stores, so
 * "Create store" is always available in the header. Every state is honest:
 * skeleton while loading, retry on error, a real empty state before the
 * first store exists.
 */
export function StoresPanel() {
  const stores = useOwnStores()
  const assortment = useStoreProducts()
  const [createOpen, setCreateOpen] = useState(false)

  // Item counts per store, derived from the assortment data.
  const productCounts = useMemo(() => {
    const counts = new Map<string, number>()
    for (const item of assortment.data ?? []) {
      counts.set(item.store_id, (counts.get(item.store_id) ?? 0) + 1)
    }
    return counts
  }, [assortment.data])

  return (
    <div>
      <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="font-heading text-2xl font-semibold tracking-tight">
            Stores
          </h2>
          <p className="mt-1.5 max-w-xl text-sm text-muted-foreground">
            Every store you run on Tuma. Open a store to manage its details,
            availability and menu.
          </p>
        </div>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus data-icon="inline-start" />
          Create store
        </Button>
      </div>

      {stores.isLoading ? (
        <div className="grid gap-6 lg:grid-cols-2">
          <Skeleton className="h-52 rounded-xl" />
          <Skeleton className="h-52 rounded-xl" />
        </div>
      ) : null}

      {stores.isError ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">Couldn&apos;t load your stores</p>
            <p className="text-sm text-muted-foreground">
              Something went wrong while fetching them.
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void stores.refetch()}
            >
              <RefreshCw data-icon="inline-start" />
              Try again
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {stores.isSuccess && stores.data.length === 0 ? (
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
                Create your first store — then add products to its menu and
                open it for orders.
              </p>
            </div>
            <Button onClick={() => setCreateOpen(true)}>
              <Plus data-icon="inline-start" />
              Create your first store
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {stores.isSuccess && stores.data.length > 0 ? (
        <div className="grid gap-6 lg:grid-cols-2">
          {stores.data.map((store) => (
            <StoreCard
              key={store.id}
              store={store}
              productCount={productCounts.get(store.id) ?? 0}
            />
          ))}
        </div>
      ) : null}

      <CreateStoreDialog open={createOpen} onOpenChange={setCreateOpen} />
    </div>
  )
}
