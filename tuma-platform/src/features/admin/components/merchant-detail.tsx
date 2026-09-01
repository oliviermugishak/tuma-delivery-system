/**
 * Admin — Merchant detail page (coverage §7.4): the single owner of a
 * business's facts AND actions (founder call: keep the page, no duplicate
 * drawer). Identity header with status dot, contact, stores list, members,
 * and the real lifecycle controls — Activate / Suspend (PATCH status,
 * guarded), Delete (guarded) — with pending states on every dialog.
 */
import { useState } from 'react'
import { Link, getRouteApi, useNavigate } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  deleteMerchantMutation,
  getMerchantOptions,
  updateMerchantMutation,
} from '@/api/queries'
import {
  Avatar,
  Button,
  Card,
  ErrorState,
  GuardDialog,
  Icon,
  PageHead,
  Status,
  TableSkeleton,
} from '@/components/ds'
import { date, initials, num, phone, rwf } from '@/lib/format'
import { merchantStatusLabel, merchantStatusTone } from '@/lib/status'
import { ApiError } from '@/api/client'

const routeApi = getRouteApi('/admin/merchants/$merchantId')

export function MerchantDetailPage() {
  const { merchantId } = routeApi.useParams()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const detail = useQuery(getMerchantOptions({ path: { id: merchantId } }))

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ['listMerchants'] })
    void queryClient.invalidateQueries({ queryKey: ['getMerchant'] })
  }

  const setStatus = useMutation({
    ...updateMerchantMutation(),
    onSuccess: (updated) => {
      invalidate()
      toast.success(
        updated.status === 'active'
          ? `${updated.name} is active again — their stores can take orders`
          : `${updated.name} suspended — their stores stop taking orders`,
      )
      setConfirm(null)
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not update the merchant'),
  })

  const removeMerchant = useMutation({
    ...deleteMerchantMutation(),
    onSuccess: () => {
      toast.success('Merchant deleted')
      invalidate()
      void navigate({ to: '/admin/merchants' })
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not delete the merchant'),
  })

  const [confirm, setConfirm] = useState<
    { kind: 'suspend' } | { kind: 'activate' } | { kind: 'delete' } | null
  >(null)

  if (detail.isLoading) {
    return (
      <div className="rounded-2xl border border-white/8 bg-surface">
        <TableSkeleton rows={6} />
      </div>
    )
  }
  if (detail.isError || !detail.data) {
    return (
      <div className="rounded-2xl border border-white/8 bg-surface">
        <ErrorState
          onRetry={() => void detail.refetch()}
          title="Couldn't load this merchant"
        />
      </div>
    )
  }
  const m = detail.data

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        back={{ to: '/admin/merchants', label: 'All merchants' }}
        title={
          <span className="flex items-center gap-3">
            <Avatar text={initials(m.name, m.business_email)} large />
            {m.name}
            <Status tone={merchantStatusTone(m.status)}>
              {merchantStatusLabel(m.status)}
            </Status>
          </span>
        }
        sub={`On Tuma since ${date(m.created_at)}`}
        actions={
          <>
            <Link to="/admin/merchants/$merchantId/edit" params={{ merchantId: m.id }}>
              <Button variant="outline">
                <Icon name="edit" label="" size={18} />
                Edit details
              </Button>
            </Link>
            {m.status === 'active' ? (
              <Button variant="dangerOutline" onClick={() => setConfirm({ kind: 'suspend' })}>
                Suspend merchant
              </Button>
            ) : (
              <Button variant="primary" onClick={() => setConfirm({ kind: 'activate' })}>
                Activate merchant
              </Button>
            )}
            <Button
              variant="dangerOutline"
              aria-label={`Delete ${m.name}`}
              onClick={() => setConfirm({ kind: 'delete' })}
            >
              <Icon name="delete" label="" size={18} />
            </Button>
          </>
        }
      />

      {m.status === 'suspended' ? (
        <div className="rounded-xl border border-danger/28 bg-danger/8 px-4 py-3.5 text-[13px] font-medium text-danger">
          Suspended — the merchant wing refuses this business and its stores
          stop taking orders. Activate to restore them.
        </div>
      ) : null}

      <div className="grid items-start gap-4 xl:grid-cols-[1fr_360px] max-xl:grid-cols-1">
        <div className="flex flex-col gap-4">
          <Card className="p-0">
            <div className="flex items-center gap-3 px-6 pt-5 pb-3">
              <div className="text-[17px] font-bold">
                Stores · {num(m.stores.length)}
              </div>
            </div>
            {m.stores.length === 0 ? (
              <div className="px-6 pb-5 text-sm text-text2">
                No stores yet — this merchant opens their first store from
                the merchant wing.
              </div>
            ) : (
              m.stores.map((s) => (
                <div
                  key={s.id}
                  className="flex items-center gap-3 border-t border-white/8 px-6 py-3 text-[13.5px]"
                >
                  <span className="min-w-0 flex-1 truncate font-semibold">
                    {s.name}
                  </span>
                  <span className="text-xs text-text3">
                    {num(s.product_count)} products
                  </span>
                  <Status tone={s.is_open ? 'success' : 'muted'} small>
                    {s.is_open ? 'Open' : 'Closed'}
                  </Status>
                  <span className="w-27.5 text-right text-text2">
                    {rwf(s.delivery_fee)}
                  </span>
                </div>
              ))
            )}
          </Card>

          <Card className="p-0">
            <div className="flex items-center gap-3 px-6 pt-5 pb-3">
              <div className="text-[17px] font-bold">
                Members · {num(m.members.length)}
              </div>
            </div>
            {m.members.length === 0 ? (
              <div className="px-6 pb-5 text-sm text-text2">
                Only the owner account signs into this business.
              </div>
            ) : (
              m.members.map((mem) => (
                <div
                  key={mem.user_id}
                  className="flex items-center gap-3 border-t border-white/8 px-6 py-3 text-[13.5px]"
                >
                  <span className="min-w-0 flex-1 truncate font-semibold">
                    {mem.email ?? 'Member'}
                  </span>
                  <span className="text-xs text-text3">
                    {mem.role}
                    {mem.store_id ? ' · one store' : ''}
                  </span>
                </div>
              ))
            )}
          </Card>
        </div>

        <div className="flex flex-col gap-4">
          <Card>
            <div className="text-[17px] font-bold">Contact</div>
            <div className="mt-2 text-[13px] text-text2">
              {m.business_email ?? 'Business email not set'}
            </div>
            <div className="text-[13px] text-text2">
              {m.business_phone
                ? phone(m.business_phone)
                : 'Business phone not set'}
            </div>
          </Card>

          <Card>
            <div className="text-[17px] font-bold">Orders</div>
            <div className="mt-2 text-sm text-text2">
              Merchants own their order lifecycle — live and past orders are
              managed from the merchant wing, so they don't appear here.
            </div>
          </Card>
        </div>
      </div>

      {/* Suspend — guarded (P10), reversible. */}
      <GuardDialog
        open={confirm?.kind === 'suspend'}
        onClose={() => setConfirm(null)}
        title={`Suspend ${m.name}?`}
        confirmLabel="Suspend merchant"
        pending={setStatus.isPending}
        onConfirm={() =>
          setStatus.mutate({ path: { id: m.id }, body: { status: 'suspended' } })
        }
        note="Suspension is reversible — activate from this page any time."
      >
        The business is refused by the merchant wing and its{' '}
        <b>{num(m.stores.length)} stores</b> stop taking orders immediately.
      </GuardDialog>

      {/* Activate — routine + reversible (P10 proportionality). */}
      <GuardDialog
        open={confirm?.kind === 'activate'}
        onClose={() => setConfirm(null)}
        title={`Activate ${m.name}?`}
        confirmLabel="Activate merchant"
        danger={false}
        pending={setStatus.isPending}
        onConfirm={() =>
          setStatus.mutate({ path: { id: m.id }, body: { status: 'active' } })
        }
      >
        The business signs back into the merchant wing and its{' '}
        <b>{num(m.stores.length)} stores</b> can take orders again.
      </GuardDialog>

      {/* Delete — the strongest guard (P10). */}
      <GuardDialog
        open={confirm?.kind === 'delete'}
        onClose={() => setConfirm(null)}
        title={`Delete ${m.name}?`}
        confirmLabel="Delete merchant"
        pending={removeMerchant.isPending}
        onConfirm={() => removeMerchant.mutate({ path: { id: m.id } })}
        note="Deletion is permanent. Prefer suspension — it keeps the business, its stores, and its assortments."
      >
        This permanently deletes the business — its{' '}
        <b>{num(m.stores.length)} stores</b> and everything they sell are
        removed with it. The owner's account itself survives; their sign-in
        just stops opening a merchant wing.
      </GuardDialog>
    </div>
  )
}
