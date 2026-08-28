import { Link } from '@tanstack/react-router'
import { ChevronRight, MapPin } from 'lucide-react'

import type { StoreResponse } from '@/api/generated'
import { Badge } from '@/components/ui/badge'
import { formatDate, formatRwf, initials } from '@/lib/format'
import { cn } from '@/lib/utils'

/**
 * One store in the merchant's portfolio: identity, status and the facts at
 * a glance, clickable through to the store's detail page. Deliberately
 * roomy — a store is a business, not a table row.
 */
export function StoreCard({
  store,
  productCount,
}: {
  store: StoreResponse
  productCount: number
}) {
  return (
    <Link
      to="/merchant/store/$storeId"
      params={{ storeId: store.id }}
      className="group block rounded-xl border bg-card p-6 transition-colors hover:border-primary/40 hover:bg-muted/30"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-4">
          <div className="flex size-12 shrink-0 items-center justify-center rounded-xl bg-primary/10 font-heading text-base font-semibold text-primary">
            {initials(store.name)}
          </div>
          <div className="grid gap-1.5">
            <div className="flex items-center gap-2.5">
              <h3 className="font-heading text-lg font-semibold">
                {store.name}
              </h3>
              <Badge
                variant="outline"
                className={cn(
                  store.is_open
                    ? 'border-transparent bg-emerald-500/15 text-emerald-600 dark:text-emerald-400'
                    : 'text-muted-foreground',
                )}
              >
                {store.is_open ? 'Open' : 'Closed'}
              </Badge>
            </div>
            <p className="flex items-center gap-1.5 text-sm text-muted-foreground">
              <MapPin className="size-3.5 shrink-0" aria-hidden />
              <span className="truncate">
                {store.address_text || 'No address yet'}
              </span>
            </p>
          </div>
        </div>
        <ChevronRight
          className="mt-1 size-5 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5 group-hover:text-foreground"
          aria-hidden
        />
      </div>

      {store.description ? (
        <p className="mt-4 line-clamp-2 text-sm text-muted-foreground">
          {store.description}
        </p>
      ) : null}

      <dl className="mt-6 flex items-center gap-8 border-t pt-4 text-sm">
        <div>
          <dt className="text-xs font-medium tracking-wider text-muted-foreground uppercase">
            Products
          </dt>
          <dd className="mt-1 font-medium">{productCount}</dd>
        </div>
        <div>
          <dt className="text-xs font-medium tracking-wider text-muted-foreground uppercase">
            Delivery fee
          </dt>
          <dd className="mt-1 font-medium">{formatRwf(store.delivery_fee)}</dd>
        </div>
        <div>
          <dt className="text-xs font-medium tracking-wider text-muted-foreground uppercase">
            Created
          </dt>
          <dd className="mt-1 font-medium">{formatDate(store.created_at)}</dd>
        </div>
      </dl>
    </Link>
  )
}
