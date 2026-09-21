/**
 * Merchant — Overview (coverage §7.10, mockup M1): KPIs and the
 * live-orders strip (one pulse, "Updated Xs ago", Accept carries the
 * total).
 *
 * Live data: /v1/merchant/orders (the board feed) + /v1/merchant/stores.
 * KPI aggregates (orders today, revenue today, items sold, avg prep
 * time) need the merchant-metrics endpoint — KPI cards carry their
 * designed empty state in words until it lands. No metrics endpoint
 * exists, so no orders chart renders here — it returns with the metrics
 * slice.
 */
import { useMemo } from 'react'
import { Link } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  advanceStoreOrderMutation,
  getMerchantStoreOrderQueryKey,
  listMerchantOrdersOptions,
  listMerchantOrdersQueryKey,
  listOwnStoresOptions,
} from '@/api/queries'
import {
  Button,
  EmptyState,
  ErrorState,
  Freshness,
  KpiCard,
  KpiGrid,
  KpiSkeleton,
  PageHead,
  Pulse,
  Skeleton,
  Status,
} from '@/components/ds'
import { num, rwf } from '@/lib/format'
import { orderStatusLabel, orderStatusTone, LIVE_ORDER_STATUSES } from '@/lib/status'
import { ApiError } from '@/api/client'

export function MerchantOverview() {
  const queryClient = useQueryClient()
  const stores = useQuery({ ...listOwnStoresOptions(), staleTime: 60_000 })
  const orders = useQuery({
    ...listMerchantOrdersOptions({ query: { limit: 50, offset: 0, status: LIVE_ORDER_STATUSES } }),
    refetchInterval: 10_000,
  })

  const live = useMemo(
    () =>
      (orders.data ?? []).filter(
        (o) => o.status !== 'delivered' && o.status !== 'cancelled',
      ),
    [orders.data],
  )
  const newest = live[0]

  const advance = useAcceptOrder(queryClient)

  const businessName = stores.data?.[0]?.name

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Overview"
        sub={
          <>
            {businessName ? `${businessName} · ` : ''}
            {new Date().toLocaleDateString('en-GB', {
              weekday: 'long',
              day: 'numeric',
              month: 'short',
            })}{' '}
            ·{' '}
            <span className="inline-flex items-center gap-1.5 text-xs font-medium text-text3">
              <Pulse />
              <Freshness
                updated={
                  orders.dataUpdatedAt
                    ? new Date(orders.dataUpdatedAt).toISOString()
                    : null
                }
              />
            </span>
          </>
        }
      />

      {/* BACKEND GAP (G9): merchant KPI aggregates — until
          GET /v1/merchant/metrics exists the cards say what they'd show
          instead of zeros (P2). */}
      {orders.isLoading ? (
        <KpiGrid>
          {Array.from({ length: 4 }).map((_, i) => (
            <KpiSkeleton key={i} />
          ))}
        </KpiGrid>
      ) : (
        <KpiGrid>
          <KpiCard
            label="Orders in progress"
            value={num(live.length)}
            vs="live on your board"
          />
          <KpiCard
            label="Newest order"
            value={newest ? `#${newest.number}` : 'None yet'}
            vs={newest ? rwf(newest.total) : 'nothing waiting'}
          />
          <KpiCard
            label="Orders today"
            value="Pending"
            vs="daily totals land with the merchant metrics slice"
          />
          <KpiCard
            label="Revenue today"
            value="Pending"
            vs="daily totals land with the merchant metrics slice"
          />
        </KpiGrid>
      )}

      <div className="rounded-2xl border border-line bg-surface p-6">
        <div className="mb-4 flex flex-wrap items-center gap-3">
          <div className="text-[17px] font-bold">Live orders</div>
          <span className="inline-flex items-center gap-1.5 text-xs font-medium text-text3">
            <Pulse />
            {live.length} in progress · auto-updates
          </span>
          <Freshness
            className="ml-auto"
            updated={
              orders.dataUpdatedAt
                ? new Date(orders.dataUpdatedAt).toISOString()
                : null
            }
          />
        </div>
        {orders.isLoading ? (
          <div className="grid gap-3 md:grid-cols-3">
            {Array.from({ length: 3 }).map((_, i) => (
              <Skeleton className="h-38" key={i} />
            ))}
          </div>
        ) : orders.isError ? (
          <ErrorState onRetry={() => void orders.refetch()} />
        ) : live.length === 0 ? (
          <EmptyState
            icon="room_service"
            title="No live orders"
            hint="New orders appear here the moment a customer checks out."
          />
        ) : (
          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
            {live.slice(0, 6).map((o) => (
              <div
                key={o.id}
                className={`flex flex-col gap-1.5 rounded-xl border p-3.5 ${
                  o.status === 'placed'
                    ? 'border-warning/50'
                    : 'border-line bg-high'
                }`}
              >
                <div className="flex items-center justify-between">
                  <span className="text-[13.5px] font-semibold text-text2">
                    #{o.number}
                  </span>
                  {o.status === 'placed' ? (
                    <span className="inline-flex items-center gap-2 text-[11.5px] font-semibold text-warning">
                      <Pulse warn />
                      New
                    </span>
                  ) : (
                    <Status tone={orderStatusTone(o.status)} small>
                      {orderStatusLabel(o.status)}
                    </Status>
                  )}
                </div>
                <div className="text-[15px] font-bold">{rwf(o.total)}</div>
                <div className="text-xs text-text3">
                  {o.store_name} · {o.address_text}
                </div>
                {o.status === 'placed' ? (
                  <Button
                    variant="primary"
                    small
                    className="mt-1"
                    disabled={advance.isPending}
                    onClick={() =>
                      advance.mutate({
                        path: { id: o.id },
                        body: { status: 'accepted' },
                      })
                    }
                  >
                    Accept · {rwf(o.total)}
                  </Button>
                ) : (
                  <Link to="/merchant/orders">
                    <Button variant="ghost" small className="mt-1 w-full">
                      View order
                    </Button>
                  </Link>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

// The overview accepts orders inline; the board owns the full flow.
function useAcceptOrder(queryClient: ReturnType<typeof useQueryClient>) {
  return useMutation({
    ...advanceStoreOrderMutation(),
    onSuccess: (order) => {
      toast.success(`Order #${order.number} accepted — start preparing`)
      void queryClient.invalidateQueries({
        queryKey: listMerchantOrdersQueryKey(),
      })
      void queryClient.invalidateQueries({
        queryKey: getMerchantStoreOrderQueryKey({ path: { id: order.id } }),
      })
    },
    onError: (e) =>
      toast.error(
        e instanceof ApiError ? e.message : 'Could not accept the order',
      ),
  })
}


