/**
 * Admin — Customers (coverage §7.6): directory with the full management
 * set the server exposes — edit name/phone (PATCH; empty clears the
 * name, phone fixes a typo'd signup, 409 on duplicates), deactivate
 * (locks out immediately, reversible), delete (guarded, permanent —
 * profile/sessions/history cascade). No create — people join via the
 * app (P3). Admins don't see orders (founder, 2026-09-01).
 */
import { useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  deleteCustomerMutation,
  listCustomersOptions,
  updateCustomerMutation,
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
import { date, initials, phone } from '@/lib/format'
import { customerStatusLabel, customerStatusTone } from '@/lib/status'
import { ApiError } from '@/api/client'

interface CustomerRow {
  user_id: string
  name?: string | null
  phone?: string | null
  is_active: boolean
  created_at: string
}

export function CustomersPage() {
  const customers = useQuery(listCustomersOptions())
  const queryClient = useQueryClient()
  const [search, setSearch] = useState('')
  const [openId, setOpenId] = useState<string | null>(null)
  const [deactivateTarget, setDeactivateTarget] = useState<CustomerRow | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<CustomerRow | null>(null)

  const setCustomer = useMutation({
    ...updateCustomerMutation(),
    onSuccess: (updated) => {
      void queryClient.invalidateQueries({ queryKey: ['listCustomers'] })
      toast.success(
        updated.is_active
          ? 'Customer reactivated — they can sign in again'
          : 'Customer deactivated — locked out immediately',
      )
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not update the customer'),
  })

  const removeCustomer = useMutation({
    ...deleteCustomerMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['listCustomers'] })
      toast.success('Customer deleted')
      setDeleteTarget(null)
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not delete the customer'),
  })

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return (customers.data ?? []).filter(
      (c) =>
        !needle ||
        (c.name ?? '').toLowerCase().includes(needle) ||
        (c.phone ?? '').includes(needle.replace(/\s/g, '')) ||
        needle === 'unnamed',
    )
  }, [customers.data, search])

  const columns: Column<CustomerRow>[] = [
    {
      key: 'customer',
      header: 'Customer',
      cell: (c) => (
        <div className="flex items-center gap-3">
          <Avatar text={initials(c.name, c.phone)} />
          <div>
            {c.name ? c.name : 'Phone verified · name not set'}
            <span className="mt-0.5 block text-[11.5px] font-medium text-text3">
              {phone(c.phone)}
            </span>
          </div>
        </div>
      ),
    },
    {
      key: 'joined',
      header: 'Joined',
      cell: (c) => <span className="text-text2">{date(c.created_at)}</span>,
    },
    {
      key: 'status',
      header: 'Status',
      cell: (c) => (
        <Status tone={customerStatusTone(c.is_active)}>
          {customerStatusLabel(c.is_active)}
        </Status>
      ),
    },
  ]

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Customers"
        sub={
          customers.data
            ? `${customers.data.length} people order on Tuma — accounts join from the mobile app`
            : 'People who order on Tuma'
        }
      />

      <Toolbar
        search={search}
        onSearch={setSearch}
        searchPlaceholder="Search by name or phone"
        resultCount={customers.data ? `${filtered.length} customers` : undefined}
      />

      {customers.isLoading ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <TableSkeleton rows={8} />
        </div>
      ) : null}
      {customers.isError ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <ErrorState onRetry={() => void customers.refetch()} />
        </div>
      ) : null}
      {customers.data && customers.data.length === 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="groups"
            title="No customers yet"
            hint="People join by ordering in the Tuma app — they'll appear here."
          />
        </div>
      ) : null}
      {customers.data && filtered.length === 0 && customers.data.length > 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="search_off"
            title="No customers match"
            hint="Try a different name or phone number."
          />
        </div>
      ) : null}
      {customers.data && filtered.length > 0 ? (
        <DataTable
          columns={columns}
          rows={filtered}
          rowKey={(c) => c.user_id}
          onRowOpen={(c) => setOpenId(c.user_id)}
        />
      ) : null}

      <CustomerDrawer
        customer={filtered.find((c) => c.user_id === openId) ?? null}
        onClose={() => setOpenId(null)}
        onDeactivate={(c) => setDeactivateTarget(c)}
        onReactivate={(c) =>
          setCustomer.mutate({ path: { id: c.user_id }, body: { is_active: true } })
        }
        onDelete={(c) => setDeleteTarget(c)}
      />

      <GuardDialog
        open={deactivateTarget != null}
        onClose={() => setDeactivateTarget(null)}
        title={`Deactivate ${deactivateTarget?.name || 'this customer'}?`}
        confirmLabel="Deactivate customer"
        onConfirm={() => {
          if (deactivateTarget) {
            setCustomer.mutate({
              path: { id: deactivateTarget.user_id },
              body: { is_active: false },
            })
          }
          setDeactivateTarget(null)
        }}
        note="Deactivation is reversible — reactivate from this drawer."
      >
        They can no longer sign in or place orders
        {deactivateTarget?.phone
          ? <> on {phone(deactivateTarget.phone)}</>
          : null}
        . Orders already in flight continue.
      </GuardDialog>

      <GuardDialog
        open={deleteTarget != null}
        onClose={() => setDeleteTarget(null)}
        title={`Delete ${deleteTarget?.name || 'this customer'}?`}
        confirmLabel="Delete customer"
        pending={removeCustomer.isPending}
        onConfirm={() => {
          if (deleteTarget)
            removeCustomer.mutate({ path: { id: deleteTarget.user_id } })
        }}
        note="Deletion is permanent — prefer deactivation, which just locks them out until reactivated."
      >
        This permanently deletes the account
        {deleteTarget?.phone ? <> on {phone(deleteTarget.phone)}</> : null} —
        their profile, sign-in sessions, and order history go with it. They
        can sign up again with the same number, starting fresh.
      </GuardDialog>
    </div>
  )
}

/**
 * Inline edit form — PATCH /v1/admin/customers/{id}. Server semantics:
 * an empty name CLEARS it; the phone must be E.164 (a taken phone is the
 * typed 409). Editing the phone fixes a typo'd OTP signup.
 */
function EditCustomerForm({
  customer,
  onDone,
}: {
  customer: CustomerRow
  onDone: () => void
}) {
  const queryClient = useQueryClient()
  const [name, setName] = useState(customer.name ?? '')
  const [phoneValue, setPhoneValue] = useState(customer.phone ?? '')
  const [errors, setErrors] = useState<Record<string, string>>({})

  const update = useMutation({
    ...updateCustomerMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['listCustomers'] })
      toast.success('Customer updated')
      onDone()
    },
    onError: (e) => {
      if (e instanceof ApiError && e.status === 409) {
        setErrors({ phone: 'That phone number already has an account' })
      } else {
        toast.error(e instanceof ApiError ? e.message : 'Could not update the customer')
      }
    },
  })

  const submit = () => {
    const next: Record<string, string> = {}
    if (phoneValue.replace(/\D/g, '').length < 9)
      next.phone = 'Enter the full international number (+250…).'
    setErrors(next)
    if (Object.keys(next).length > 0) return

    const body: { name?: string; phone?: string } = {}
    if (name.trim() !== (customer.name ?? '')) body.name = name.trim() // "" clears
    if (phoneValue.trim() !== (customer.phone ?? '')) body.phone = phoneValue.trim()
    if (Object.keys(body).length === 0) {
      onDone()
      return
    }
    update.mutate({ path: { id: customer.user_id }, body })
  }

  return (
    <DrawerSection label="Account details">
      <Field
        label="Name"
        help="Leave empty to clear — they can set it again from the app"
        error={errors.name}
      >
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Phone verified · name not set"
        />
      </Field>
      <Field
        label="Phone"
        help="Their sign-in anchor. International format."
        error={errors.phone}
      >
        <Input
          type="tel"
          value={phoneValue}
          onChange={(e) => setPhoneValue(e.target.value)}
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

function CustomerDrawer({
  customer,
  onClose,
  onDeactivate,
  onReactivate,
  onDelete,
}: {
  customer: CustomerRow | null
  onClose: () => void
  onDeactivate: (c: CustomerRow) => void
  onReactivate: (c: CustomerRow) => void
  onDelete: (c: CustomerRow) => void
}) {
  const [editing, setEditing] = useState(false)

  if (!customer) return null
  return (
    <Drawer
      open
      onClose={onClose}
      title={
        <span className="flex items-center gap-2.5">
          <Avatar text={initials(customer.name, customer.phone)} large />
          {editing
            ? 'Edit customer'
            : customer.name
              ? customer.name
              : 'Unnamed customer'}
        </span>
      }
      subtitle={`${phone(customer.phone)} · joined ${date(customer.created_at)}`}
      status={
        editing ? undefined : (
          <Status tone={customerStatusTone(customer.is_active)}>
            {customerStatusLabel(customer.is_active)}
          </Status>
        )
      }
      footer={
        editing ? undefined : (
          <>
            <Button
              variant="outline"
              className="flex-1"
              onClick={() => setEditing(true)}
            >
              Edit customer
            </Button>
            {customer.is_active ? (
              <Button
                variant="dangerOutline"
                onClick={() => {
                  onDeactivate(customer)
                  onClose()
                }}
              >
                Deactivate
              </Button>
            ) : (
              <Button
                variant="primary"
                onClick={() => {
                  onReactivate(customer)
                  onClose()
                }}
              >
                Reactivate
              </Button>
            )}
            <Button
              variant="dangerOutline"
              aria-label={`Delete ${customer.name || 'customer'}`}
              onClick={() => {
                onDelete(customer)
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
        <EditCustomerForm customer={customer} onDone={() => setEditing(false)} />
      ) : (
        <>
          <DrawerSection label="Identity">
            <div className="text-[13px] text-text2">
              {customer.name
                ? customer.name
                : "Phone verified · name not set — they haven't finished their profile."}
            </div>
          </DrawerSection>

          <DrawerSection label="Sign-in">
            <div className="text-[13px] text-text2">{phone(customer.phone)}</div>
            <div className="text-xs text-text3">
              The phone is the OTP anchor — editing it fixes a typo'd signup
              number.
            </div>
          </DrawerSection>

          <DrawerSection label="Order history">
            {/* Admins don't see orders (founder, 2026-09-01) — the
                customer's history lives in their own app. */}
            <div className="text-[13px] text-text2">
              Merchants own the order lifecycle — history isn't shown in the
              admin wing.
            </div>
          </DrawerSection>
        </>
      )}
    </Drawer>
  )
}
