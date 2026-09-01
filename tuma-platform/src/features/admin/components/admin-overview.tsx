/**
 * Admin — Overview (coverage §7.1, mockup A1). Live platform counts come
 * straight from /v1/admin/summary; the revenue chart and the payout /
 * dispute / onboarding rows of "Needs attention" ride on demo seed rows
 * (BACKEND-GAPS.md G3/G4/G8/G9) until those services exist — never fake
 * numbers inside real cards.
 */
import { useState } from 'react'
import { Link } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'

import { summaryOptions } from '@/api/queries'
import {
  Button,
  ChartCard,
  ErrorState,
  Freshness,
  Icon,
  KpiCard,
  KpiGrid,
  KpiSkeleton,
  LineChart,
  PageHead,
  Pulse,
} from '@/components/ds'
import { demoDisputes, demoRevenue7d } from '@/features/demo/seed'
import { num, rwf } from '@/lib/format'

export function AdminOverview() {
  const summary = useQuery({ ...summaryOptions(), refetchInterval: 20_000 })
  const [range, setRange] = useState<'today' | '7d' | '30d'>('today')
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
        actions={
          <>
            <RangeChip on={range === 'today'} onClick={() => setRange('today')}>
              Today
            </RangeChip>
            <RangeChip on={range === '7d'} onClick={() => setRange('7d')}>
              7 days
            </RangeChip>
            <RangeChip on={range === '30d'} onClick={() => setRange('30d')}>
              30 days
            </RangeChip>
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

      <div className="grid items-start gap-4 xl:grid-cols-[1fr_424px] max-xl:grid-cols-1">
        <ChartCard
          title="Revenue"
          summary={
            <>
              Last 7 days ·{' '}
              <b className="font-bold text-foreground">
                {rwf(demoRevenue7d.reduce((s, p) => s + p.value, 0))}
              </b>
            </>
          }
        >
          {/* BACKEND GAP (G9): no metrics endpoint — demo series until
              GET /v1/admin/metrics lands. */}
          <LineChart
            points={demoRevenue7d.map((p) => ({ label: p.date.split(' ')[0], date: p.date, value: p.value }))}
            color="accent"
            formatValue={(v) => `${num(v)} RWF`}
          />
        </ChartCard>

        <NeedsAttention />
      </div>
    </div>
  )
}

function RangeChip({
  on,
  onClick,
  children,
}: {
  on: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={on}
      className={`inline-flex h-8 items-center gap-1.5 rounded-full border px-3 text-[13px] font-medium whitespace-nowrap transition-colors duration-150 ${
        on
          ? 'border-brand/45 bg-brand/7 text-brand'
          : 'border-line text-text2 hover:text-foreground'
      }`}
    >
      {children}
    </button>
  )
}

/**
 * Rows end in a verb (P17). Real service-backed rows first (today: only
 * the disputes gap-shower); the stale demo payouts / pending-review rows
 * were removed with seed.ts's trim (founder, 2026-09-01 — no such server
 * state exists).
 */
function NeedsAttention() {
  const openDisputes = demoDisputes.filter((d) => d.status === 'open')
  const rows = [
    openDisputes.length > 0
      ? {
          key: 'disputes',
          tone: 'danger' as const,
          title: `${openDisputes.length} open disputes`,
          titleBold: null as string | null,
          sub: `${rwf(openDisputes.reduce((s, d) => s + d.amount, 0))} at stake`,
          cta: 'Open disputes',
          primary: false,
          to: '/admin/disputes',
        }
      : null,
  ].filter((r): r is NonNullable<typeof r> => r != null)

  return (
    <div className="rounded-2xl border border-white/8 bg-surface p-6">
      <div className="mb-2 flex items-center gap-3">
        <div className="text-[17px] font-bold">Needs attention</div>
        {rows.length > 0 ? (
          <span className="ml-auto rounded-full bg-high px-2 py-0.5 text-[11px] font-bold text-text3">
            {rows.length}
          </span>
        ) : null}
      </div>
      {rows.length === 0 ? (
        <div className="flex items-center gap-2 py-8 text-sm text-text2">
          <Icon name="task_alt" label="" size={18} className="text-success" />
          Nothing needs you right now.
        </div>
      ) : (
        rows.map((row) => (
          <div
            key={row.key}
            className="flex items-center gap-3 border-t border-white/8 py-3.5 first:border-t-0"
          >
            <span
              className={`size-2 shrink-0 rounded-full ${
                row.tone === 'danger' ? 'bg-danger' : 'bg-warning'
              }`}
            />
            <div className="min-w-0 flex-1">
              <div className="text-[13.5px] font-semibold">
                {row.titleBold ? `${row.title} · ${row.titleBold}` : row.title}
              </div>
              <div className="mt-0.5 text-xs text-text3">{row.sub}</div>
            </div>
            <Link to={row.to}>
              <Button variant="ghost" small>
                {row.cta}
              </Button>
            </Link>
          </div>
        ))
      )}
    </div>
  )
}
