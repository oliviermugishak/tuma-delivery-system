/**
 * Admin — Riders (coverage §7.5): table Rider · Phone · Deliveries
 * today · On-time % · Status; drawer = the full edit surface (name/phone
 * PATCH, deactivate guarded, reactivate instant, delete guarded — the
 * hard delete is permanent; prefer deactivation, which keeps history);
 * "Add rider" = page.
 *
 * Live data: /v1/admin/riders. Deliveries-today / on-time % / live
 * "On delivery" status need the rider-stats endpoint (G10) — those
 * columns render their designed empty state in words until then.
 */
import { useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  deleteRiderMutation,
  listRidersOptions,
  updateRiderMutation,
} from '@/api/queries'
import {
  Avatar,
  Button,
  DataTable,
  Drawer,
  DrawerSection,
  EmptyState,
  ErrorState,
  Field,
  GuardDialog,
  Icon,
  Input,
  PageHead,
  Status,
  TableSkeleton,
  Toolbar,
  type Column,
} from '@/components/ds'
import { demoRiderStats } from '@/features/demo/seed'
import { date, initials, num, phone } from '@/lib/format'
import { riderStatusLabel, riderStatusTone } from '@/lib/status'
import { ApiError } from '@/api/client'

interface RiderRow {
  id: string
  name: string
  phone: string
  rider_number: number
  is_active: boolean
  created_at: string
}

export function RidersPage() {
  const riders = useQuery(listRidersOptions())
  const queryClient = useQueryClient()
  const [search, setSearch] = useState('')
  const [openId, setOpenId] = useState<string | null>(null)
  const [deactivateTarget, setDeactivateTarget] = useState<RiderRow | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<RiderRow | null>(null)

  const setRider = useMutation({
    ...updateRiderMutation(),
    onSuccess: (updated) => {
      void queryClient.invalidateQueries({ queryKey: ['listRiders'] })
      toast.success(
        updated.is_active
          ? `${updated.name} is available for hand-offs again`
          : 'Rider deactivated — they stay signed in but get no hand-offs',
      )
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not update the rider'),
  })

  const removeRider = useMutation({
    ...deleteRiderMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['listRiders'] })
      toast.success('Rider deleted')
      setDeleteTarget(null)
    },
    onError: (e) => {
      setDeleteTarget(null)
      toast.error(e instanceof ApiError ? e.message : 'Could not delete the rider')
    },
  })

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return (riders.data ?? []).filter(
      (r) =>
        !needle ||
        r.name.toLowerCase().includes(needle) ||
        r.phone.includes(needle) ||
        String(r.rider_number) === needle,
    )
  }, [riders.data, search])

  const columns: Column<RiderRow>[] = [
    {
      key: 'rider',
      header: 'Rider',
      cell: (r) => (
        <div className="flex items-center gap-3">
          <Avatar text={initials(r.name, String(r.rider_number))} />
          <div>
            {r.name}
            <span className="mt-0.5 block text-[11.5px] font-medium text-text3">
              Rider #{r.rider_number}
            </span>
          </div>
        </div>
      ),
    },
    {
      key: 'phone',
      header: 'Phone',
      cell: (r) => <span className="text-text2">{phone(r.phone)}</span>,
    },
    {
      key: 'today',
      header: 'Deliveries today',
      numeric: true,
      // BACKEND GAP (G10): GET /v1/admin/riders/stats
      cell: (r) =>
        demoRiderStats[String(r.rider_number)] ? (
          num(demoRiderStats[String(r.rider_number)].deliveriesToday)
        ) : (
          <span className="text-text3">No deliveries yet</span>
        ),
    },
    {
      key: 'ontime',
      header: 'On-time %',
      numeric: true,
      // BACKEND GAP (G10).
      cell: (r) =>
        demoRiderStats[String(r.rider_number)] ? (
          `${demoRiderStats[String(r.rider_number)].onTimePct}%`
        ) : (
          <span className="text-text3">Not enough runs</span>
        ),
    },
    {
      key: 'status',
      header: 'Status',
      cell: (r) => {
        const onDelivery = demoRiderStats[String(r.rider_number)]?.onDelivery
        return (
          <Status
            tone={onDelivery ? 'accent' : riderStatusTone(r.is_active)}
          >
            {onDelivery ? 'On delivery' : riderStatusLabel(r.is_active)}
          </Status>
        )
      },
    },
  ]

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Riders"
        sub={
          riders.data
            ? `${riders.data.length} riders · ${riders.data.filter((r) => r.is_active).length} available`
            : 'The fleet fulfilling deliveries'
        }
        actions={
          <Link to="/admin/riders/new">
            <Button variant="primary">
              <Icon name="add" label="" size={18} />
              Add rider
            </Button>
          </Link>
        }
      />

      <Toolbar
        search={search}
        onSearch={setSearch}
        searchPlaceholder="Search riders by name, phone, number"
        resultCount={riders.data ? `${filtered.length} riders` : undefined}
      />

      {riders.isLoading ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <TableSkeleton rows={6} />
        </div>
      ) : null}
      {riders.isError ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <ErrorState onRetry={() => void riders.refetch()} />
        </div>
      ) : null}
      {riders.data && riders.data.length === 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="two_wheeler"
            title="No riders yet"
            hint="Add the first rider — they sign in with their phone and an OTP code."
            action={
              <Link to="/admin/riders/new">
                <Button small variant="primary">
                  Add rider
                </Button>
              </Link>
            }
          />
        </div>
      ) : null}
      {riders.data && filtered.length === 0 && riders.data.length > 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="search_off"
            title="No riders match"
            hint="Try a different name, phone, or rider number."
          />
        </div>
      ) : null}
      {riders.data && filtered.length > 0 ? (
        <DataTable
          columns={columns}
          rows={filtered}
          rowKey={(r) => r.id}
          onRowOpen={(r) => setOpenId(r.id)}
        />
      ) : null}

      <RiderDrawer
        rider={filtered.find((r) => r.id === openId) ?? null}
        onClose={() => setOpenId(null)}
        onDeactivate={(r) => setDeactivateTarget(r)}
        onReactivate={(r) =>
          setRider.mutate({ path: { id: r.id }, body: { is_active: true } })
        }
        onDelete={(r) => setDeleteTarget(r)}
      />

      <GuardDialog
        open={deactivateTarget != null}
        onClose={() => setDeactivateTarget(null)}
        title={`Deactivate ${deactivateTarget?.name ?? ''}?`}
        confirmLabel="Deactivate rider"
        onConfirm={() => {
          if (deactivateTarget) {
            setRider.mutate({
              path: { id: deactivateTarget.id },
              body: { is_active: false },
            })
          }
          setDeactivateTarget(null)
        }}
        note="Deactivation is reversible — reactivate from this drawer any time."
      >
        The rider can still sign in, but merchants can't hand orders to
        them — Rider #{deactivateTarget?.rider_number ?? ''} disappears from
        hand-off until reactivated.
      </GuardDialog>

      <GuardDialog
        open={deleteTarget != null}
        onClose={() => setDeleteTarget(null)}
        title={`Delete ${deleteTarget?.name ?? ''}?`}
        confirmLabel="Delete rider"
        pending={removeRider.isPending}
        onConfirm={() => {
          if (deleteTarget) removeRider.mutate({ path: { id: deleteTarget.id } })
        }}
        note="Riders with delivery history can't be deleted — deactivate them instead. Deletion is permanent for everyone else."
      >
        This permanently deletes Rider #{deleteTarget?.rider_number ?? ''}{' '}
        and their sign-in account.
      </GuardDialog>
    </div>
  )
}

function RiderDrawer({
  rider,
  onClose,
  onDeactivate,
  onReactivate,
  onDelete,
}: {
  rider: RiderRow | null
  onClose: () => void
  onDeactivate: (r: RiderRow) => void
  onReactivate: (r: RiderRow) => void
  onDelete: (r: RiderRow) => void
}) {
  const [editing, setEditing] = useState(false)
  const [name, setName] = useState('')
  const [phoneValue, setPhoneValue] = useState('')
  const [errors, setErrors] = useState<Record<string, string>>({})

  if (!rider) return null
  const stats = demoRiderStats[String(rider.rider_number)]

  const startEdit = () => {
    setName(rider.name)
    setPhoneValue(rider.phone)
    setErrors({})
    setEditing(true)
  }

  return (
    <Drawer
      open
      onClose={onClose}
      title={
        <span className="flex items-center gap-2.5">
          <Avatar text={initials(rider.name)} large />
          {editing ? 'Edit rider' : rider.name}
        </span>
      }
      subtitle={`Rider #${rider.rider_number} · ${phone(rider.phone)} · joined ${date(rider.created_at)}`}
      status={
        editing ? undefined : (
          <Status tone={riderStatusTone(rider.is_active)}>
            {riderStatusLabel(rider.is_active)}
          </Status>
        )
      }
      footer={
        editing ? undefined : (
          <>
            <Button variant="outline" className="flex-1" onClick={startEdit}>
              Edit rider
            </Button>
            {rider.is_active ? (
              <Button
                variant="dangerOutline"
                onClick={() => {
                  onDeactivate(rider)
                  onClose()
                }}
              >
                Deactivate
              </Button>
            ) : (
              <Button
                variant="primary"
                onClick={() => {
                  onReactivate(rider)
                  onClose()
                }}
              >
                Reactivate
              </Button>
            )}
            <Button
              variant="dangerOutline"
              aria-label={`Delete ${rider.name}`}
              onClick={() => {
                onDelete(rider)
                onClose()
              }}
            >
              <Icon name="delete" label="" size={18} />
            </Button>
          </>
        )
      }
    >
      {editing ? (
        <EditRiderForm
          rider={rider}
          name={name}
          phoneValue={phoneValue}
          errors={errors}
          onName={setName}
          onPhone={setPhoneValue}
          onErrors={setErrors}
          onDone={() => setEditing(false)}
        />
      ) : (
        <>
          <DrawerSection label="Current order">
            {/* BACKEND GAP (G10): the rider's live delivery needs
                GET /v1/admin/riders/{id}/current-delivery. */}
            {stats?.onDelivery ? (
              <div className="text-[13px] text-text2">
                On delivery right now — live tracking lives with the order.
              </div>
            ) : (
              <div className="text-[13px] text-text2">
                {rider.is_active
                  ? 'Waiting for a hand-off — no order in their hands.'
                  : 'Off duty — not receiving hand-offs.'}
              </div>
            )}
          </DrawerSection>

          <DrawerSection label="Recent deliveries">
            <div className="text-[13px] text-text2">
              {stats
                ? `${stats.deliveriesToday} deliveries today · ${stats.onTimePct}% on time.`
                : 'No completed deliveries recorded yet — history appears after their first run.'}
            </div>
          </DrawerSection>

          <DrawerSection label="Sign-in">
            <div className="text-[13px] text-text2">{phone(rider.phone)}</div>
            <div className="text-xs text-text3">
              The rider signs in with this phone and an OTP code — there's no
              password. Editing the phone changes where their codes go.
            </div>
          </DrawerSection>
        </>
      )}
    </Drawer>
  )
}

/**
 * Inline edit form — PATCH /v1/admin/riders/{id} with optional
 * name/phone; a taken phone is the server's typed 409, surfaced verbatim.
 */
function EditRiderForm({
  rider,
  name,
  phoneValue,
  errors,
  onName,
  onPhone,
  onErrors,
  onDone,
}: {
  rider: RiderRow
  name: string
  phoneValue: string
  errors: Record<string, string>
  onName: (v: string) => void
  onPhone: (v: string) => void
  onErrors: (e: Record<string, string>) => void
  onDone: () => void
}) {
  const queryClient = useQueryClient()
  const update = useMutation({
    ...updateRiderMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['listRiders'] })
      toast.success('Rider updated')
      onDone()
    },
    onError: (e) => {
      if (e instanceof ApiError && e.status === 409) {
        onErrors({ phone: 'That phone number already has a rider' })
      } else {
        toast.error(e instanceof ApiError ? e.message : 'Could not update the rider')
      }
    },
  })

  const submit = () => {
    const next: Record<string, string> = {}
    if (!name.trim()) next.name = 'The rider needs a name.'
    if (phoneValue.replace(/\D/g, '').length < 9)
      next.phone = 'Enter the full international number (+250…).'
    onErrors(next)
    if (Object.keys(next).length > 0) return

    const body: { name?: string; phone?: string } = {}
    if (name.trim() !== rider.name) body.name = name.trim()
    if (phoneValue.trim() !== rider.phone) body.phone = phoneValue.trim()
    if (Object.keys(body).length === 0) {
      onDone()
      return
    }
    update.mutate({ path: { id: rider.id }, body })
  }

  return (
    <DrawerSection label="Rider details">
      <Field label="Full name" error={errors.name}>
        <Input
          value={name}
          onChange={(e) => onName(e.target.value)}
          invalid={!!errors.name}
        />
      </Field>
      <Field
        label="Phone"
        help="Their sign-in — the OTP anchor. International format."
        error={errors.phone}
      >
        <Input
          type="tel"
          value={phoneValue}
          onChange={(e) => onPhone(e.target.value)}
          invalid={!!errors.phone}
        />
      </Field>
      <div className="flex gap-2.5">
        <Button
          variant="primary"
          small
          onClick={submit}
          disabled={update.isPending}
        >
          {update.isPending ? 'Saving…' : 'Save changes'}
        </Button>
        <Button variant="ghost" small onClick={onDone}>
          Cancel
        </Button>
      </div>
    </DrawerSection>
  )
}
