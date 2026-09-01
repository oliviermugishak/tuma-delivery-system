/**
 * Admin — Resolve dispute (coverage §7.7): a PAGE (P18). Outcome =
 * Refund X RWF (guard restating money + payout impact), Reject with
 * reason, or Partial refund. Every dispute ends decided (P17);
 * resolutions write to the audit log (G6 note).
 *
 * BACKEND GAP (G3): POST /v1/admin/disputes/{id}/resolve — the outcome
 * stays client-side until the disputes service exists.
 */
import { useState } from 'react'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { toast } from 'sonner'

import {
  Button,
  Card,
  Field,
  GuardDialog,
  Input,
  PageHead,
  Status,
  Textarea,
} from '@/components/ds'
import { demoDisputes } from '@/features/demo/seed'
import { dateTime, rwf } from '@/lib/format'

type Outcome = 'refund' | 'partial' | 'reject'

const routeApi = getRouteApi('/admin/disputes/$disputeId')

export function DisputeResolve() {
  const { disputeId } = routeApi.useParams()
  const navigate = useNavigate()
  const dispute = demoDisputes.find((d) => d.id === disputeId)

  const [outcome, setOutcome] = useState<Outcome>('refund')
  const [partial, setPartial] = useState('')
  const [reason, setReason] = useState('')
  const [confirmOpen, setConfirmOpen] = useState(false)
  const [errors, setErrors] = useState<Record<string, string>>({})

  if (!dispute) {
    return (
      <div className="rounded-2xl border border-white/8 bg-surface">
        <div className="px-6 py-14 text-center">
          <div className="text-[15px] font-bold">Dispute not found</div>
          <div className="mt-1 text-[13px] text-text2">
            It may already be decided.
          </div>
          <div className="mt-4">
            <Button small onClick={() => void navigate({ to: '/admin/disputes' })}>
              Back to disputes
            </Button>
          </div>
        </div>
      </div>
    )
  }

  const amount =
    outcome === 'partial' ? Number(partial.replace(/\D/g, '')) : dispute.amount

  const submit = () => {
    const next: Record<string, string> = {}
    if (outcome === 'partial' && (!amount || amount <= 0 || amount > dispute.amount))
      next.partial = `Enter an amount between 1 and ${dispute.amount} RWF.`
    if ((outcome === 'reject' || outcome === 'partial') && !reason.trim())
      next.reason = 'Give the customer a reason.'
    setErrors(next)
    if (Object.keys(next).length === 0) setConfirmOpen(true)
  }

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        back={{ to: '/admin/disputes', label: 'Disputes & Refunds' }}
        title={`Resolve dispute #${dispute.number}`}
        sub={`Order #${dispute.orderNumber} · ${dispute.store} · ${dispute.customer} · opened ${dateTime(dispute.openedAt)}`}
        actions={<Status tone="warning">Open</Status>}
      />

      <div className="grid items-start gap-4 xl:grid-cols-[1fr_380px] max-xl:grid-cols-1">
        <Card>
          <div className="text-[17px] font-bold">Decision</div>
          <div className="mt-4 flex flex-col gap-2">
            <OutcomeOption
              checked={outcome === 'refund'}
              onSelect={() => setOutcome('refund')}
              title={`Full refund · ${rwf(dispute.amount)}`}
              hint="The customer gets every franc back, including the delivery fee."
            />
            <OutcomeOption
              checked={outcome === 'partial'}
              onSelect={() => setOutcome('partial')}
              title="Partial refund"
              hint="Refund part of the order — say how much and why."
            />
            {outcome === 'partial' ? (
              <div className="ml-1">
                <Field
                  label="Refund amount (RWF)"
                  error={errors.partial}
                  help={`At most ${rwf(dispute.amount)} — the order's full value`}
                >
                  <Input
                    value={partial}
                    onChange={(e) => setPartial(e.target.value)}
                    placeholder={String(dispute.amount)}
                    inputMode="numeric"
                    invalid={!!errors.partial}
                  />
                </Field>
              </div>
            ) : null}
            <OutcomeOption
              checked={outcome === 'reject'}
              onSelect={() => setOutcome('reject')}
              title="Reject"
              hint="No money moves — the customer is told why."
            />
            {outcome === 'partial' || outcome === 'reject' ? (
              <div className="ml-1">
                <Field
                  label={outcome === 'reject' ? 'Reason' : 'Reason for the customer'}
                  error={errors.reason}
                >
                  <Textarea
                    rows={3}
                    value={reason}
                    onChange={(e) => setReason(e.target.value)}
                    placeholder="Written to the customer, word for word."
                  />
                </Field>
              </div>
            ) : null}
          </div>

          <div className="mt-5 flex gap-3">
            <Button variant="primary" onClick={submit}>
              Record decision
            </Button>
            <Button variant="ghost" onClick={() => void navigate({ to: '/admin/disputes' })}>
              Cancel
            </Button>
          </div>
        </Card>

        <Card>
          <div className="text-[17px] font-bold">Trail</div>
          {dispute.events.map((e, i) => (
            <div key={i} className="border-t border-white/8 py-2.5 first:border-t-0">
              <div className="text-[13.5px] font-semibold">{e.actor}</div>
              <div className="text-[13px] text-text2">{e.text}</div>
              <div className="mt-0.5 text-xs text-text3">{dateTime(e.at)}</div>
            </div>
          ))}
        </Card>
      </div>

      <GuardDialog
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        title={
          outcome === 'reject'
            ? `Reject dispute #${dispute.number}?`
            : `Refund ${rwf(amount)} to ${dispute.customer}?`
        }
        confirmLabel={outcome === 'reject' ? 'Reject dispute' : `Refund ${rwf(amount)}`}
        onConfirm={() => {
          // BACKEND GAP (G3): POST /v1/admin/disputes/{id}/resolve
          // { outcome, amount?, reason } — writes the audit-log entry (G6).
          setConfirmOpen(false)
          toast.success(
            outcome === 'reject'
              ? 'Dispute rejected — the customer is notified with your reason'
              : `Refunding ${rwf(amount)} — the customer and ${dispute.store} are notified`,
          )
          void navigate({ to: '/admin/disputes' })
        }}
        note="The decision is final — the customer and the store are notified immediately. This can't be undone."
      >
        {outcome === 'reject' ? (
          <>
            No money moves. {dispute.customer} is told the dispute is
            rejected, with your reason.
          </>
        ) : (
          <>
            This refunds <b>{rwf(amount)}</b> to {dispute.customer}.
            {amount < dispute.amount ? (
              <>
                {' '}The remaining <b>{rwf(dispute.amount - amount)}</b> stays
                with {dispute.store}.
              </>
            ) : (
              <>
                {' '}{dispute.store}'s next payout is reduced by{' '}
                <b>{rwf(dispute.amount)}</b>.
              </>
            )}
          </>
        )}
      </GuardDialog>
    </div>
  )
}

function OutcomeOption({
  checked,
  onSelect,
  title,
  hint,
}: {
  checked: boolean
  onSelect: () => void
  title: string
  hint: string
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={checked}
      onClick={onSelect}
      className={`flex items-start gap-3 rounded-xl border px-4 py-3 text-left transition-colors duration-150 ${
        checked ? 'border-brand/45 bg-brand/7' : 'border-line hover:bg-white/4'
      }`}
    >
      <span
        className={`mt-0.5 grid size-4.5 shrink-0 place-items-center rounded-full border-[1.5px] ${
          checked ? 'border-brand' : 'border-line'
        }`}
      >
        {checked ? <span className="size-2 rounded-full bg-brand" /> : null}
      </span>
      <span>
        <span className="block text-sm font-semibold">{title}</span>
        <span className="mt-0.5 block text-xs text-text2">{hint}</span>
      </span>
    </button>
  )
}
