/**
 * Merchant — Earnings & Payouts (coverage §7.15, mockup M5): hero
 * balance + "Request payout · X RWF" (eligible only), held-for-open-
 * orders note, payout table; failed payout row shows reason + Retry.
 *
 * BACKEND GAP (G4): no payouts or earnings-ledger endpoints. The
 * balance hero, request action, and payout rows ride on demo seed rows
 * (features/demo/seed.ts) until the payments milestone — the request
 * button says so instead of pretending (P2/P3).
 */
import { useState } from 'react'
import { toast } from 'sonner'

import {
  Button,
  DataTable,
  EmptyState,
  Freshness,
  GuardDialog,
  Icon,
  PageHead,
  Pulse,
  Status,
  type Column,
} from '@/components/ds'
import {
  demoMerchantPayouts,
  payoutLabel,
  payoutTone,
  type DemoPayout,
} from '@/features/demo/seed'
import { date, num, rwf } from '@/lib/format'

export function EarningsScreen() {
  const [payouts, setPayouts] = useState<DemoPayout[]>(demoMerchantPayouts)
  const [requestOpen, setRequestOpen] = useState(false)
  const [retryTarget, setRetryTarget] = useState<DemoPayout | null>(null)

  // BACKEND GAP (G4): balance comes from the earnings ledger —
  // GET /v1/merchant/earnings/balance. The demo numbers below exist so
  // the flow is reviewable end-to-end; they are not server truth.
  const available = 218_000
  const held = 32_500

  const columns: Column<DemoPayout>[] = [
    { key: 'payout', header: 'Payout', cell: (p) => <span className="font-semibold">#{p.id.replace('m', '')}</span> },
    { key: 'period', header: 'Period', cell: (p) => <span className="text-text2">{p.period}</span> },
    { key: 'amount', header: 'Amount (RWF)', numeric: true, cell: (p) => num(p.amount) },
    {
      key: 'status',
      header: 'Status',
      cell: (p) => (
        <Status tone={payoutTone(p.status)}>{payoutLabel(p.status)}</Status>
      ),
    },
    { key: 'date', header: 'Date', cell: (p) => <span className="text-text2">{date(p.requestedAt)}</span> },
    {
      key: 'action',
      header: '',
      cell: (p) =>
        p.status === 'failed' ? (
          <Button variant="outline" small onClick={() => setRetryTarget(p)}>
            <Icon name="refresh" label="" size={16} />
            Retry payout
          </Button>
        ) : undefined,
    },
  ]

  const failReason = (p: DemoPayout | null) =>
    p?.failureReason ? (
      <div className="mb-1 text-[13px] text-danger">{p.failureReason}</div>
    ) : null

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Earnings & Payouts"
        sub={
          <>
            Cash sales minus platform fees ·{' '}
            <span className="inline-flex items-center gap-1.5 text-xs font-medium text-text3">
              <Pulse />
              <Freshness updated={new Date().toISOString()} />
            </span>
          </>
        }
        actions={
          <Button variant="primary" onClick={() => setRequestOpen(true)}>
            <Icon name="payments" label="" size={18} />
            Request payout · {rwf(available)}
          </Button>
        }
      />

      {/* BACKEND GAP (G4): the whole hero is demo money. */}
      <div className="flex flex-wrap items-start gap-12 rounded-2xl border border-white/8 bg-surface p-6 max-md:gap-8">
        <div>
          <div className="text-[11px] font-semibold tracking-[0.09em] uppercase text-text2">
            Available balance
          </div>
          <div className="mt-2.5 flex items-baseline gap-2 text-[32px] font-extrabold tracking-tight text-brand">
            {num(available)}
            <span className="text-[15px] font-semibold text-text2">RWF</span>
          </div>
          <div className="mt-1.5 text-[13px] text-text2">
            Excludes {rwf(held)} held for open orders
          </div>
        </div>
        <div>
          <div className="text-[11px] font-semibold tracking-[0.09em] uppercase text-text2">
            This week · net
          </div>
          <div className="mt-2 text-lg font-bold">612,000 RWF</div>
          <div className="mt-0.5 text-xs text-text3">
            Cash sales minus platform fees
          </div>
        </div>
        <div>
          <div className="text-[11px] font-semibold tracking-[0.09em] uppercase text-text2">
            Last payout
          </div>
          <div className="mt-2 text-lg font-bold">
            {payouts.find((p) => p.status === 'paid')
              ? `${num(payouts.find((p) => p.status === 'paid')!.amount)} RWF`
              : 'No payout yet'}
          </div>
          <div className="mt-0.5 text-xs text-text3">
            {payouts.find((p) => p.status === 'paid')
              ? `Paid ${date(payouts.find((p) => p.status === 'paid')!.requestedAt)}`
              : 'Requested payouts appear here'}
          </div>
        </div>
      </div>

      <div>
        <div className="mb-3 flex items-center gap-3">
          <div className="text-[17px] font-bold">Payouts</div>
        </div>
        {payouts.length === 0 ? (
          <div className="rounded-2xl border border-white/8 bg-surface">
            <EmptyState
              icon="payments"
              title="No payouts yet"
              hint="Request your first payout once the balance is ready."
            />
          </div>
        ) : (
          <DataTable
            columns={columns}
            rows={payouts}
            rowKey={(p) => p.id}
            empty={null}
          />
        )}
      </div>

      {/* Request guard — money moves (P10), CTA carries the amount (P8). */}
      <GuardDialog
        open={requestOpen}
        onClose={() => setRequestOpen(false)}
        title="Request payout?"
        confirmLabel={`Request · ${rwf(available)}`}
        danger={false}
        onConfirm={() => {
          // BACKEND GAP (G4): POST /v1/merchant/payouts — demo row only.
          setPayouts((prev) => [
            {
              id: `m${prev.length + 48}`,
              merchant: 'KFC Rwanda',
              amount: available,
              requestedAt: new Date().toISOString(),
              status: 'pending',
              period: 'This week',
            },
            ...prev,
          ])
          setRequestOpen(false)
          toast.error('Demo — payouts need the payments service (BACKEND-GAPS.md G4)')
        }}
        note="The request goes to Tuma Admin for approval — money lands after approval."
      >
        This asks Tuma to pay <b>{rwf(available)}</b> to your payout
        account. The <b>{rwf(held)}</b> held for open orders stays until
        those orders finish.
      </GuardDialog>

      {/* Retry — restates the failure (P10 proportionality: routine). */}
      <GuardDialog
        open={retryTarget != null}
        onClose={() => setRetryTarget(null)}
        title={`Retry payout · ${rwf(retryTarget?.amount ?? 0)}?`}
        confirmLabel={`Retry · ${rwf(retryTarget?.amount ?? 0)}`}
        danger={false}
        onConfirm={() => {
          // BACKEND GAP (G4): POST /v1/merchant/payouts/{id}/retry.
          setPayouts((prev) =>
            prev.map((p) =>
              p.id === retryTarget?.id ? { ...p, status: 'processing' as const } : p,
            ),
          )
          setRetryTarget(null)
          toast.error('Demo — payout retries need the payments service (BACKEND-GAPS.md G4)')
        }}
      >
        {failReason(retryTarget)}
        The payout for <b>{rwf(retryTarget?.amount ?? 0)}</b>
        {' '}({retryTarget?.period}) will be processed again with your
        current payout details.
      </GuardDialog>
    </div>
  )
}
