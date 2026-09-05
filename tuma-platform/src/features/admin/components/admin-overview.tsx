/**
 * Admin — Overview (coverage §7.1, mockup A1). Live platform counts come
 * straight from /v1/admin/summary. No metrics endpoint exists, so no
 * revenue chart renders here — it returns with the metrics slice.
 */
import { useQuery } from '@tanstack/react-query'

import { summaryOptions } from '@/api/queries'
import {
  ErrorState,
  Freshness,
  KpiCard,
  KpiGrid,
  KpiSkeleton,
  PageHead,
  Pulse,
} from '@/components/ds'
import { num } from '@/lib/format'

export function AdminOverview() {
  const summary = useQuery({ ...summaryOptions(), refetchInterval: 20_000 })
  const now = new Date().toISOString()

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Overview"
        sub={
          <>
            {new Date().toLocaleDateString('en-GB', {
              weekday: 'long',
              day: 'numeric',
              month: 'short',
            })}{' '}
            ·{' '}
            <span className="inline-flex items-center gap-1.5 text-xs font-medium text-text3">
              <Pulse />
              Live counts · <Freshness updated={summary.dataUpdatedAt ? new Date(summary.dataUpdatedAt).toISOString() : now} />
            </span>
          </>
        }
      />

      {summary.isLoading ? (
        <KpiGrid>
          {Array.from({ length: 4 }).map((_, i) => (
            <KpiSkeleton key={i} />
          ))}
        </KpiGrid>
      ) : null}
      {summary.isError ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <ErrorState onRetry={() => void summary.refetch()} />
        </div>
      ) : null}
      {summary.data ? (
        <KpiGrid>
          <KpiCard
            label="Sellable products"
            value={num(summary.data.store_products)}
            vs={`from ${num(summary.data.products)} catalog products`}
          />
          <KpiCard label="Merchants" value={num(summary.data.merchants)} vs={`${num(summary.data.stores)} stores on the platform`} />
          <KpiCard
            label="Stores open now"
            value={num(summary.data.open_stores)}
            vs={`of ${num(summary.data.stores)} stores`}
          />
          <KpiCard label="Customers" value={num(summary.data.customers)} vs="accounts on the platform" />
        </KpiGrid>
      ) : null}
    </div>
  )
}
