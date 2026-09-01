/**
 * Admin — Merchants (coverage §7.3, mockup A3). Every server capability
 * on /v1/admin/merchants is reachable: create (page), view (the detail
 * PAGE — the single owner of profile facts and actions, founder call:
 * keep page), edit (page), activate / suspend (PATCH status — activation
 * is instant, suspension is guarded, P10), delete (guarded). Bulk bar
 * covers batch suspend. The list response carries no store counts —
 * those live on the detail (GET .../{id}), so the table shows only facts
 * the list actually owns (P2).
 */
import { useMemo, useState } from 'react'
import { Link, useNavigate } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  deleteMerchantMutation,
  listMerchantsOptions,
  updateMerchantMutation,
} from '@/api/queries'
import {
  Button,
  BulkBar,
  Chip,
  DataTable,
  EmptyState,
  ErrorState,
  GuardDialog,
  Icon,
  Kebab,
  PageHead,
  Status,
  TableSkeleton,
  Toolbar,
  type Column,
} from '@/components/ds'
import { date } from '@/lib/format'
import { merchantStatusLabel, merchantStatusTone } from '@/lib/status'
import { ApiError } from '@/api/client'
import { downloadCsv, toCsv } from '@/lib/csv'

type MerchantRow = {
  id: string
  name: string
  business_email?: string | null
  business_phone?: string | null
  status: string
  created_at: string
}

export function MerchantsPage() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const merchants = useQuery(listMerchantsOptions())
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | 'active' | 'suspended'>('all')
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [suspendTargets, setSuspendTargets] = useState<MerchantRow[] | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<MerchantRow | null>(null)

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ['listMerchants'] })
    void queryClient.invalidateQueries({ queryKey: ['getMerchant'] })
  }

  const removeMerchant = useMutation({
    ...deleteMerchantMutation(),
    onSuccess: () => {
      toast.success('Merchant deleted')
      invalidate()
      setDeleteTarget(null)
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not delete the merchant'),
  })

  // One mutation serves both directions of the status PATCH; activation is
  // routine (instant + toast), suspension is the guarded direction (P10).
  const setStatus = useMutation({
    ...updateMerchantMutation(),
    onSuccess: (updated) => {
      invalidate()
      toast.success(
        updated.status === 'active'
          ? `${updated.name} is active again — their stores can take orders`
          : `${updated.name} suspended — their stores stop taking orders`,
      )
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not update the merchant'),
  })

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return (merchants.data ?? []).filter((m) => {
      if (statusFilter !== 'all' && m.status !== statusFilter) return false
      if (!needle) return true
      return (
        m.name.toLowerCase().includes(needle) ||
        (m.business_email ?? '').toLowerCase().includes(needle)
      )
    })
  }, [merchants.data, search, statusFilter])

  const columns: Column<MerchantRow>[] = [
    {
      key: 'merchant',
      header: 'Merchant',
      cell: (m) => (
        <div>
          {m.name}
          <span className="mt-0.5 block text-[11.5px] font-medium text-text3">
            {m.business_email ?? 'Business email not set'}
          </span>
        </div>
      ),
    },
    {
      key: 'phone',
      header: 'Business phone',
      cell: (m) => (
        <span className="text-text2">{m.business_phone ?? 'Not set'}</span>
      ),
    },
    {
      key: 'since',
      header: 'On Tuma since',
      cell: (m) => <span className="text-text2">{date(m.created_at)}</span>,
    },
    {
      key: 'status',
      header: 'Status',
      cell: (m) => (
        <Status tone={merchantStatusTone(m.status)}>
          {merchantStatusLabel(m.status)}
        </Status>
      ),
    },
    {
      key: 'actions',
      header: '',
      cell: (m) => (
        <Kebab
          items={[
            {
              label: 'View profile',
              icon: 'visibility',
              onSelect: () =>
                void navigate({ to: `/admin/merchants/${m.id}` }),
            },
            {
              label: 'Edit details',
              icon: 'edit',
              onSelect: () =>
                void navigate({ to: `/admin/merchants/${m.id}/edit` }),
            },
          ]}
        />
      ),
    },
  ]

  const exportCsv = () => {
    const csv = toCsv(
      [
        { header: 'Merchant', value: (m: MerchantRow) => m.name },
        { header: 'Business email', value: (m: MerchantRow) => m.business_email ?? '' },
        { header: 'Business phone', value: (m: MerchantRow) => m.business_phone ?? '' },
        { header: 'Status', value: (m: MerchantRow) => m.status },
        { header: 'Since', value: (m: MerchantRow) => date(m.created_at) },
      ] as never,
      filtered as never,
    )
    downloadCsv('tuma-merchants.csv', csv)
  }

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Merchants"
        sub={
          merchants.data
            ? `${merchants.data.length} businesses · ${
                merchants.data.filter((m) => m.status === 'suspended').length
              } suspended`
            : 'The businesses selling on Tuma'
        }
        actions={
          <Link to="/admin/merchants/new">
            <Button variant="primary">
              <Icon name="add" label="" size={18} />
              Add merchant
            </Button>
          </Link>
        }
      />

      <Toolbar
        search={search}
        onSearch={setSearch}
        searchPlaceholder="Search merchants"
        resultCount={merchants.data ? `${filtered.length} of ${merchants.data.length}` : undefined}
        actions={
          <Button variant="outline" small onClick={exportCsv}>
            <Icon name="download" label="" size={16} />
            Export CSV
          </Button>
        }
      >
        <Chip
          on={statusFilter === 'active'}
          onClick={() => setStatusFilter(statusFilter === 'active' ? 'all' : 'active')}
        >
          Active
        </Chip>
        <Chip
          on={statusFilter === 'suspended'}
          onClick={() =>
            setStatusFilter(statusFilter === 'suspended' ? 'all' : 'suspended')
          }
        >
          Suspended
        </Chip>
      </Toolbar>

      {merchants.isLoading ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <TableSkeleton rows={6} />
        </div>
      ) : null}
      {merchants.isError ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <ErrorState onRetry={() => void merchants.refetch()} />
        </div>
      ) : null}
      {merchants.data && filtered.length === 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="storefront"
            title={merchants.data.length === 0 ? 'No merchants yet' : 'No merchants match'}
            hint={
              merchants.data.length === 0
                ? 'Add the first business — they get an owner account instantly.'
                : 'Adjust the search or filters.'
            }
            action={
              merchants.data.length === 0 ? (
                <Link to="/admin/merchants/new">
                  <Button small variant="primary">
                    Add merchant
                  </Button>
                </Link>
              ) : undefined
            }
          />
        </div>
      ) : null}
      {merchants.data && filtered.length > 0 ? (
        <DataTable
          columns={columns}
          rows={filtered}
          rowKey={(m) => m.id}
          onRowOpen={(m) => void navigate({ to: `/admin/merchants/${m.id}` })}
          selectable
          selected={selected}
          onSelectedChange={setSelected}
        />
      ) : null}

      <BulkBar
        count={selected.size}
        onClear={() => setSelected(new Set())}
        note="Bulk actions apply to every selected merchant"
      >
        <Button variant="outline" small onClick={exportCsv}>
          Export
        </Button>
        <Button
          variant="dangerOutline"
          small
          onClick={() => {
            setSuspendTargets(
              filtered.filter((m) => selected.has(m.id) && m.status === 'active'),
            )
          }}
        >
          Suspend
        </Button>
      </BulkBar>

      {/* Suspend guard — restates scale (P10). */}
      <GuardDialog
        open={suspendTargets != null}
        onClose={() => setSuspendTargets(null)}
        title={
          suspendTargets?.length === 1
            ? `Suspend ${suspendTargets[0].name}?`
            : `Suspend ${suspendTargets?.length ?? 0} merchants?`
        }
        confirmLabel={
          suspendTargets?.length === 1 ? 'Suspend merchant' : 'Suspend merchants'
        }
        pending={setStatus.isPending}
        onConfirm={() => {
          // Sequential PATCH /v1/admin/merchants/{id} per target — the
          // V1 scale makes batching unnecessary.
          suspendTargets?.forEach((m) => {
            setStatus.mutate({ path: { id: m.id }, body: { status: 'suspended' } })
          })
          setSuspendTargets(null)
          setSelected(new Set())
        }}
        note="Suspension is reversible — activate the merchant any time from their profile."
      >
        A suspended business is refused by the merchant wing and its stores
        stop taking orders immediately
        {suspendTargets?.length === 1
          ? <> — <b>{suspendTargets[0].name}</b> and everything it sells go dark.</>
          : <> — <b>{suspendTargets?.length ?? 0} businesses</b> go dark.</>}
      </GuardDialog>

      {/* Delete guard — the strongest guard (P10). */}
      <GuardDialog
        open={deleteTarget != null}
        onClose={() => setDeleteTarget(null)}
        title={deleteTarget ? `Delete ${deleteTarget.name}?` : 'Delete merchant?'}
        confirmLabel="Delete merchant"
        pending={removeMerchant.isPending}
        onConfirm={() => {
          if (deleteTarget) removeMerchant.mutate({ path: { id: deleteTarget.id } })
        }}
        note="Deletion is permanent. Prefer suspension — it keeps the business, its stores, and its assortments."
      >
        This permanently deletes the business — its stores and everything
        they sell are removed with it. The owner's account itself survives;
        their sign-in just stops opening a merchant wing.
      </GuardDialog>
    </div>
  )
}
