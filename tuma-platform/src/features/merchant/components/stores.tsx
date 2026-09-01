/**
 * Merchant — Stores (coverage §7.13, mockup M3's Stores cards): cards
 * with dot+text status, product counts, orders in progress (from the
 * live order feed — not "created"), delivery fee. "New store" = page.
 *
 * Live data: /v1/merchant/stores + /v1/merchant/store-products +
 * /v1/merchant/orders.
 */
import { useMemo } from 'react'
import { Link, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'

import {
  listMerchantOrdersOptions,
  listOwnStoresOptions,
  listStoreProductsOptions,
} from '@/api/queries'
import {
  Button,
  EmptyState,
  ErrorState,
  Icon,
  PageHead,
  Skeleton,
  Status,
} from '@/components/ds'
import { num, rwf } from '@/lib/format'
import { storeStatusLabel, storeStatusTone } from '@/lib/status'

export function StoresScreen() {
  const navigate = useNavigate()
  const stores = useQuery(listOwnStoresOptions())
  const products = useQuery(listStoreProductsOptions())
  const orders = useQuery({
    ...listMerchantOrdersOptions({ query: { limit: 50, offset: 0 } }),
    refetchInterval: 15_000,
  })

  const counts = useMemo(() => {
    const productsByStore = new Map<string, number>()
    for (const p of products.data ?? []) {
      productsByStore.set(p.store_id, (productsByStore.get(p.store_id) ?? 0) + 1)
    }
    const ordersByStore = new Map<string, number>()
    for (const o of orders.data ?? []) {
      if (o.status !== 'delivered' && o.status !== 'cancelled') {
        ordersByStore.set(o.store_id, (ordersByStore.get(o.store_id) ?? 0) + 1)
      }
    }
    return { productsByStore, ordersByStore }
  }, [products.data, orders.data])

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Stores"
        sub={
          stores.data
            ? `${stores.data.length} stores · customers only see stores that are open for orders`
            : 'Your storefronts on Tuma'
        }
        actions={
          <Link to="/merchant/store/new">
            <Button variant="primary">
              <Icon name="add" label="" size={18} />
              New store
            </Button>
          </Link>
        }
      />

      {stores.isLoading ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 3 }).map((_, i) => (
            <Skeleton className="h-56" key={i} />
          ))}
        </div>
      ) : null}
      {stores.isError ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <ErrorState onRetry={() => void stores.refetch()} />
        </div>
      ) : null}
      {stores.data && stores.data.length === 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="storefront"
            title="No stores yet"
            hint="A store is where customers order from — give it a name, an address, and a pin on the map."
            action={
              <Link to="/merchant/store/new">
                <Button small variant="primary">
                  New store
                </Button>
              </Link>
            }
          />
        </div>
      ) : null}
      {stores.data && stores.data.length > 0 ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {stores.data.map((store) => (
            <div
              key={store.id}
              role="button"
              tabIndex={0}
              onClick={() => void navigate({ to: `/merchant/store/${store.id}` })}
              onKeyDown={(e) => {
                if (e.key === 'Enter')
                  void navigate({ to: `/merchant/store/${store.id}` })
              }}
              className="flex cursor-pointer flex-col gap-2 rounded-2xl border border-white/8 bg-surface p-5 transition-colors duration-150 hover:bg-white/4"
            >
              <div className="flex items-center gap-2.5">
                <span className="grid size-10 shrink-0 place-items-center rounded-[10px] bg-high text-text3">
                  {store.image_url ? (
                    <img src={store.image_url} alt="" className="size-full rounded-[10px] object-cover" />
                  ) : (
                    <Icon name="storefront" label="" size={18} />
                  )}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[15px] font-bold">{store.name}</div>
                  <div className="truncate text-xs text-text3">
                    {store.category ?? 'Category not set'}
                  </div>
                </div>
                <Icon name="chevron_right" label="" size={18} className="text-text3" />
              </div>

              <Status tone={storeStatusTone(store.is_open)}>
                {storeStatusLabel(store.is_open)}
                {store.is_open ? ' for orders' : ''}
              </Status>

              <div className="mt-1 grid grid-cols-2 gap-2 text-[13px]">
                <div>
                  <div className="text-text3">Products</div>
                  <div className="font-semibold">
                    {num(counts.productsByStore.get(store.id) ?? 0)}
                  </div>
                </div>
                <div>
                  <div className="text-text3">Orders in progress</div>
                  <div className="font-semibold">
                    {num(counts.ordersByStore.get(store.id) ?? 0)}
                  </div>
                </div>
              </div>

              <div className="mt-auto flex items-center justify-between border-t border-white/8 pt-3 text-[13px]">
                <span className="text-text3">Delivery fee</span>
                <span className="font-semibold">
                  {store.delivery_fee === 0 ? 'Free delivery' : rwf(store.delivery_fee)}
                </span>
              </div>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  )
}
