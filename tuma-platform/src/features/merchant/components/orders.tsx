/**
 * Merchant — Orders (coverage §7.11, mockup M2): Live board (To accept /
 * Preparing / Ready for hand-off / Handed off — stage-scoped verbs,
 * designed empty columns) + History tab with receipt actions; cancelled
 * shows the cash-collection state.
 *
 * Live data: /v1/merchant/orders + advance + handoff (all real). The
 * merchant machine: placed→accepted (Accept) →preparing (Mark ready is
 * NOT a server state — see below) …
 *
 * Server truth (commerce/src/orders.rs): placed→accepted→preparing→
 * picked_up(handoff)→delivered, cancelled from the first three. The
 * mockup's "Ready for hand-off" column maps to `preparing` orders whose
 * items are done — the platform marks them ready by handing off, so the
 * board's third column shows preparing orders with the Hand off verb.
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  advanceStoreOrderMutation,
  getMerchantStoreOrderOptions,
  getMerchantStoreOrderQueryKey,
  handoffStoreOrderMutation,
  listMerchantOrdersOptions,
  listMerchantOrdersQueryKey,
} from '@/api/queries'
import {
  Button,
  CopyableId,
  DataTable,
  Drawer,
  DrawerSection,
  EmptyState,
  ErrorState,
  Freshness,
  GuardDialog,
  Input,
  PageHead,
  Pulse,
  Skeleton,
  Status,
  TableSkeleton,
  Tabs,
  type Column,
} from '@/components/ds'
import type { MerchantStoreOrderResponse } from '@/api/generated'
import { dateTime, num, phone, rwf } from '@/lib/format'
import { lookupHandoff, recordHandoff } from '@/lib/handoff-log'
import { isMuted, notifyOrder, playPing, placedArrivals } from '@/lib/order-alerts'
import {
  orderStatusLabel,
  orderStatusTone,
  paymentStatusLabel,
  paymentStatusTone,
  LIVE_ORDER_STATUSES,
  SETTLED_ORDER_STATUSES,
} from '@/lib/status'
import { ApiError } from '@/api/client'
import { Textarea } from '@/components/ui/textarea'

export function OrdersScreen() {
  const [tab, setTab] = useState<'board' | 'history'>('board')
  // Two server-filtered feeds (W1.5): the Live board asks the server for
  // the in-flight statuses, History for the settled ones — each is its
  // own cache entry, and neither competes with the other for page space.
  const liveOrders = useQuery({
    ...listMerchantOrdersOptions({ query: { limit: 50, offset: 0, status: LIVE_ORDER_STATUSES } }),
    refetchInterval: 10_000,
  })
  const settledOrders = useQuery({
    ...listMerchantOrdersOptions({ query: { limit: 50, offset: 0, status: SETTLED_ORDER_STATUSES } }),
    refetchInterval: 30_000,
  })

  // New-order arrivals (W3.1): the live board's poll is the detection
  // surface. First load seeds the seen-set silently — never alert for
  // orders already on the board; every later load diffs `placed` ids
  // against it and announces each arrival (toast + mutable ping + pop-up
  // when permission was granted).
  const seenRef = useRef(new Set<string>())
  const initializedRef = useRef(false)
  useEffect(() => {
    const data = liveOrders.data
    if (data === undefined) return
    if (!initializedRef.current) {
      initializedRef.current = true
      for (const order of data) seenRef.current.add(order.id)
      return
    }
    const arrivalIds = placedArrivals(data, seenRef.current)
    for (const order of data) seenRef.current.add(order.id)
    for (const id of arrivalIds) {
      const order = data.find((o) => o.id === id)
      if (!order) continue
      toast.success(`New order #${order.number} — ${order.store_name}`)
      if (!isMuted()) playPing()
      notifyOrder('New Tuma order', `#${order.number} — ${order.store_name}`)
    }
  }, [liveOrders.data])

  const live = useMemo(
    () =>
      (liveOrders.data ?? []).filter(
        (o) => o.status !== 'delivered' && o.status !== 'cancelled',
      ),
    [liveOrders.data],
  )
  const history = useMemo(() => settledOrders.data ?? [], [settledOrders.data])

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Orders"
        sub={
          <>
            Live across your stores ·{' '}
            <span className="inline-flex items-center gap-1.5 text-xs font-medium text-text3">
              <Pulse />
              Auto-updates ·{' '}
              <Freshness
                updated={
                  liveOrders.dataUpdatedAt
                    ? new Date(liveOrders.dataUpdatedAt).toISOString()
                    : null
                }
              />
            </span>
          </>
        }
      />

      <Tabs
        tabs={[
          { key: 'board', label: 'Live board' },
          { key: 'history', label: 'History' },
        ]}
        active={tab}
        onChange={(k) => setTab(k as 'board' | 'history')}
      />

      {liveOrders.isError ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <ErrorState onRetry={() => void liveOrders.refetch()} />
        </div>
      ) : tab === 'board' ? (
        <Board
          orders={live}
          loading={liveOrders.isLoading}
        />
      ) : (
        <History orders={history} loading={settledOrders.isLoading} />
      )}
    </div>
  )
}

/* ------------------------------------------------------------------ */

const COLUMNS = [
  { key: 'placed', title: 'To accept', statuses: ['placed'] },
  { key: 'preparing', title: 'Preparing', statuses: ['accepted'] },
  { key: 'ready', title: 'Ready for hand-off', statuses: ['preparing'] },
  { key: 'handed', title: 'Handed off', statuses: ['picked_up'] },
] as const

function Board({
  orders,
  loading,
}: {
  orders: MerchantStoreOrderResponse[]
  loading: boolean
}) {
  const queryClient = useQueryClient()
  const [detailId, setDetailId] = useState<string | null>(null)

  const invalidate = () => {
    void queryClient.invalidateQueries({
      queryKey: listMerchantOrdersQueryKey(),
    })
    if (detailId) {
      void queryClient.invalidateQueries({
        queryKey: getMerchantStoreOrderQueryKey({ path: { id: detailId } }),
      })
    }
  }

  const advance = useMutation({
    ...advanceStoreOrderMutation(),
    onSuccess: (order) => {
      toast.success(`Order #${order.number} → ${orderStatusLabel(order.status)}`)
      invalidate()
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not update the order'),
  })

  const handoff = useMutation({
    ...handoffStoreOrderMutation(),
    onSuccess: (order, variables) => {
      // Record the operator's own input so the sheet can show who the
      // order went to (the server response carries no rider identity —
      // INTEGRATION-NOTES.md finding 1).
      recordHandoff(variables.path.id, variables.body.rider_number)
      toast.success(
        `Order #${order.number} handed to rider #${variables.body.rider_number} — out for delivery`,
      )
      invalidate()
    },
    onError: (e) =>
      toast.error(
        e instanceof ApiError ? e.message : 'Could not hand the order to the rider',
      ),
  })

  return (
    <>
      {loading ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton className="h-85" key={i} />
          ))}
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {COLUMNS.map((col) => {
            const colOrders = orders.filter((o) =>
              (col.statuses as readonly string[]).includes(o.status),
            )
            return (
              <div
                key={col.key}
                className="flex min-h-85 flex-col gap-2.5 rounded-2xl border border-white/8 bg-surface p-3.5"
                aria-label={col.title}
              >
                <div className="flex items-center gap-2 text-[11px] font-semibold tracking-[0.07em] uppercase text-text2">
                  {col.key === 'placed' ? (
                    <Pulse warn />
                  ) : (
                    <i
                      className={`size-2 rounded-full ${
                        col.key === 'handed' ? 'bg-text3' : 'bg-brand'
                      }`}
                    />
                  )}
                  {col.title}
                  <span className="ml-auto rounded-full bg-high px-2 py-0.5 text-[11px] font-bold text-text3">
                    {colOrders.length}
                  </span>
                </div>

                {colOrders.length === 0 ? (
                  <div className="my-auto px-3 py-6 text-center">
                    <span className="ms block text-text3" style={{ fontSize: 28 }}>
                      {col.key === 'handed'
                        ? 'pedal_bike'
                        : col.key === 'placed'
                          ? 'notifications_off'
                          : 'skillet'}
                    </span>
                    <div className="mt-2 text-[13px] font-semibold text-text2">
                      {col.key === 'handed'
                        ? 'Nothing waiting'
                        : col.key === 'placed'
                          ? 'No new orders'
                          : 'Nothing here'}
                    </div>
                    <div className="mt-0.5 text-xs text-text3">
                      {col.key === 'handed'
                        ? 'Handed-off orders move to History.'
                        : col.key === 'placed'
                          ? 'New orders land here the moment they’re placed.'
                          : 'Orders move here as you work the board.'}
                    </div>
                  </div>
                ) : (
                  colOrders.map((o) => (
                    <OrderCard
                      key={o.id}
                      order={o}
                      onOpen={() => setDetailId(o.id)}
                      onPrimary={() => {
                        if (o.status === 'placed') {
                          advance.mutate({
                            path: { id: o.id },
                            body: { status: 'accepted' },
                          })
                        } else if (o.status === 'accepted') {
                          advance.mutate({
                            path: { id: o.id },
                            body: { status: 'preparing' },
                          })
                        } else if (o.status === 'preparing') {
                          setDetailId(o.id)
                        }
                      }}
                      busy={advance.isPending}
                    />
                  ))
                )}
              </div>
            )
          })}
        </div>
      )}

      <OrderDetail
        id={detailId}
        onClose={() => setDetailId(null)}
        onHandoff={(id, riderNumber) =>
          handoff.mutate({ path: { id }, body: { rider_number: riderNumber } })
        }
        handoffPending={handoff.isPending}
      />
    </>
  )
}

function OrderCard({
  order,
  onOpen,
  onPrimary,
  busy,
}: {
  order: MerchantStoreOrderResponse
  onOpen: () => void
  onPrimary: () => void
  busy: boolean
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => {
        if (e.key === 'Enter') onOpen()
      }}
      className={`flex cursor-pointer flex-col gap-1.5 rounded-xl border p-3.5 text-left transition-colors duration-150 hover:bg-white/4 ${
        order.status === 'placed'
          ? 'border-warning/50'
          : 'border-line bg-high'
      }`}
    >
      <div className="flex items-center justify-between">
        <span className="text-[13.5px] font-semibold text-text2">
          #{order.number}
        </span>
        <Status tone={orderStatusTone(order.status)} small>
          {order.status === 'preparing' ? 'Ready' : orderStatusLabel(order.status)}
        </Status>
      </div>
      <div className="text-[15px] font-bold">{rwf(order.total)}</div>
      <div className="text-xs text-text3">
        {order.store_name} · {order.address_text}
      </div>
      {order.status === 'picked_up' && lookupHandoff(order.id) != null ? (
        <div className="text-xs font-semibold text-text2">
          With rider #{lookupHandoff(order.id)}
        </div>
      ) : null}
      {order.status === 'picked_up' ? (
        <Button variant="ghost" small className="mt-1" onClick={(e) => { e.stopPropagation(); onOpen() }}>
          View order
        </Button>
      ) : (
        <Button
          variant={order.status === 'placed' ? 'primary' : 'outline'}
          small
          className="mt-1"
          disabled={busy}
          onClick={(e) => {
            e.stopPropagation()
            onPrimary()
          }}
        >
          {order.status === 'placed'
            ? `Accept · ${rwf(order.total)}`
            : order.status === 'accepted'
              ? 'Start preparing'
              : 'Hand off'}
        </Button>
      )}
    </div>
  )
}

/**
 * The fulfillment-sheet drawer: items, money, customer, and the handoff
 * (rider number typed in — the whole assignment interface).
 */
function OrderDetail({
  id,
  onClose,
  onHandoff,
  handoffPending,
}: {
  id: string | null
  onClose: () => void
  onHandoff: (id: string, riderNumber: number) => void
  handoffPending: boolean
}) {
  const detail = useQuery({
    ...getMerchantStoreOrderOptions({ path: { id: id ?? '' } }),
    enabled: id != null,
  })
  const [riderNumber, setRiderNumber] = useState('')
  const [cancelOpen, setCancelOpen] = useState(false)

  if (id == null) return null
  const d = detail.data

  return (
    <>
      <Drawer
        open
        onClose={onClose}
        title={d ? <>Order <CopyableId id={d.number} /></> : 'Order'}
        subtitle={d ? `Placed ${dateTime(d.created_at)} · Cash on delivery` : undefined}
        status={
          d ? (
            <Status tone={orderStatusTone(d.status)}>
              {orderStatusLabel(d.status)}
            </Status>
          ) : undefined
        }
        footer={
          d &&
          (d.status === 'preparing' ||
            d.status === 'accepted' ||
            // The server allows re-assignment until delivery: handoff
            // reruns on a picked_up order (orders.rs handoff docs).
            (d.status === 'picked_up' && lookupHandoff(d.id) != null)) ? (
            <>
              <Button
                variant="primary"
                className="flex-1"
                disabled={handoffPending || !riderNumber}
                onClick={() => {
                  const n = Number(riderNumber)
                  if (n) {
                    onHandoff(id, n)
                    setRiderNumber('')
                  }
                }}
              >
                {d.status === 'picked_up'
                  ? 'Re-assign rider'
                  : 'Hand off to rider'}
              </Button>
              {d.status !== 'picked_up' ? (
                <Button variant="dangerOutline" onClick={() => setCancelOpen(true)}>
                  Cancel order
                </Button>
              ) : null}
            </>
          ) : undefined
        }
      >
        {!d ? (
          <div className="flex flex-col gap-3">
            <Skeleton className="h-5 w-40" />
            <Skeleton className="h-20 w-full" />
            <Skeleton className="h-20 w-full" />
          </div>
        ) : (
          <>
            <DrawerSection label="Customer">
              <div className="text-sm font-semibold">
                {d.customer_name ?? 'Phone verified · name not set'}
              </div>
              <div className="text-[13px] text-text2">{phone(d.customer_phone)}</div>
              <div className="text-[13px] text-text2">{d.address_text}</div>
            </DrawerSection>

            <DrawerSection label={`Items · ${d.store_name}`}>
              {d.items.map((item) => (
                <div
                  key={item.product_name}
                  className="flex justify-between gap-3 py-1 text-[13.5px]"
                >
                  <span>
                    {item.product_name}
                    <span className="ml-1.5 text-xs text-text3">
                      ×{item.quantity}
                    </span>
                  </span>
                  <span>{num(item.unit_price * item.quantity)}</span>
                </div>
              ))}
              <div className="flex justify-between gap-3 py-1 text-[13.5px]">
                <span>Delivery fee</span>
                <span>{num(d.delivery_fee)}</span>
              </div>
              <div className="mt-1.5 flex justify-between border-t border-white/8 pt-3 text-[15px] font-bold">
                <span>Total to collect</span>
                <span className="text-brand">{rwf(d.total)}</span>
              </div>
            </DrawerSection>

            <DrawerSection label="Rider">
              {(() => {
                // Server truth first: the sheet response carries no rider
                // identity yet (INTEGRATION-NOTES.md finding 1) — what we
                // can show honestly is the number this operator typed at
                // hand-off, and only while the order is in rider hands.
                const assigned =
                  d.status === 'picked_up' ? lookupHandoff(d.id) : null
                if (assigned != null) {
                  return (
                    <div className="text-sm font-semibold">
                      Handed to rider #{assigned}
                      <span className="mt-0.5 block text-xs font-medium text-text3">
                        Re-hand-off to another rider any time before delivery.
                      </span>
                    </div>
                  )
                }
                if (d.status === 'delivered' || d.status === 'cancelled') {
                  return (
                    <div className="text-[13px] text-text2">
                      {d.status === 'delivered'
                        ? 'Delivered — the final rider assignment is frozen.'
                        : 'No rider — the order was cancelled before hand-off.'}
                    </div>
                  )
                }
                return (
                  <div className="text-[13px] text-text2">
                    Type the rider's unique number — it's on their rider card.
                    The assignment is instant and can be redone until delivery.
                  </div>
                )
              })()}
              {d.status === 'picked_up' ? (
                <Input
                  value={riderNumber}
                  inputMode="numeric"
                  onChange={(e) => setRiderNumber(e.target.value.replace(/\D/g, ''))}
                  placeholder="New rider number to re-assign"
                  aria-label="New rider number"
                />
              ) : d.status === 'accepted' || d.status === 'preparing' ? (
                <Input
                  value={riderNumber}
                  inputMode="numeric"
                  onChange={(e) => setRiderNumber(e.target.value.replace(/\D/g, ''))}
                  placeholder="Rider number"
                  aria-label="Rider number"
                />
              ) : null}
            </DrawerSection>

            <DrawerSection label="Payment">
              <Status tone={paymentStatusTone(d.payment_status)}>
                {paymentStatusLabel(d.payment_status)}
              </Status>
            </DrawerSection>
          </>
        )}
      </Drawer>

      <CancelGuard
        open={cancelOpen}
        orderId={id}
        number={d?.number}
        total={d?.total ?? 0}
        onClose={() => setCancelOpen(false)}
        onDone={() => {
          setCancelOpen(false)
          onClose()
        }}
      />
    </>
  )
}

function CancelGuard({
  open,
  orderId,
  number,
  total,
  onClose,
  onDone,
}: {
  open: boolean
  orderId: string
  number?: number
  total: number
  onClose: () => void
  onDone: () => void
}) {
  const queryClient = useQueryClient()
  const [reason, setReason] = useState('')
  const cancel = useMutation({
    ...advanceStoreOrderMutation(),
    onSuccess: () => {
      // The board and the sheet must reflect the cancel immediately —
      // same invalidation contract as the board's advance mutation.
      void queryClient.invalidateQueries({ queryKey: listMerchantOrdersQueryKey() })
      void queryClient.invalidateQueries({
        queryKey: getMerchantStoreOrderQueryKey({ path: { id: orderId } }),
      })
      toast.success(
        number
          ? `Order #${number} cancelled — mark the cash as refunded in History`
          : 'Order cancelled — mark the cash as refunded in History',
      )
      onDone()
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not cancel the order'),
  })

  return (
    <GuardDialog
      open={open}
      onClose={() => {
        setReason('')
        onClose()
      }}
      title={number ? `Cancel order #${number}?` : 'Cancel this order?'}
      confirmLabel={`Cancel order · ${rwf(total)}`}
      pending={cancel.isPending}
      onConfirm={() => {
        // The reason travels with the status and lands on the order —
        // the customer's detail and the receipt both render it.
        cancel.mutate({
          path: { id: orderId },
          body: { status: 'cancelled', reason: reason.trim() || null },
        })
      }}
      note="The customer is notified. Cash already taken must be returned — the order keeps its collection state. This can't be undone."
    >
      <div className="flex flex-col gap-3">
        This cancels the order worth <b>{rwf(total)}</b>. The customer is
        told immediately, and the money returns to them.
        <Textarea
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          maxLength={500}
          rows={3}
          placeholder="Reason (optional) — the customer sees this, e.g. 'ran out of stock'"
        />
      </div>
    </GuardDialog>
  )
}

/* History — delivered + cancelled, receipt actions, cash state. */
function History({
  orders,
  loading,
}: {
  orders: MerchantStoreOrderResponse[]
  loading: boolean
}) {
  const [receiptId, setReceiptId] = useState<string | null>(null)

  const columns: Column<MerchantStoreOrderResponse>[] = [
    {
      key: 'order',
      header: 'Order',
      cell: (o) => <CopyableId id={o.number} />,
    },
    {
      key: 'store',
      header: 'Store',
      cell: (o) => o.store_name,
    },
    {
      key: 'placed',
      header: 'Placed',
      cell: (o) => <span className="text-text2">{dateTime(o.created_at)}</span>,
    },
    {
      key: 'total',
      header: 'Total (RWF)',
      numeric: true,
      cell: (o) => num(o.total),
    },
    {
      key: 'status',
      header: 'Status',
      cell: (o) => (
        <Status tone={orderStatusTone(o.status)}>
          {orderStatusLabel(o.status)}
        </Status>
      ),
    },
    {
      key: 'receipt',
      header: '',
      cell: (o) => (
        <Button
          variant="ghost"
          small
          aria-label={`View receipt for order #${o.number}`}
          onClick={() => setReceiptId(o.id)}
        >
          <span className="ms s18" aria-hidden>
            receipt_long
          </span>
          Receipt
        </Button>
      ),
    },
  ]

  return (
    <>
      {loading ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <TableSkeleton rows={6} />
        </div>
      ) : orders.length === 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="history"
            title="No finished orders yet"
            hint="Delivered and cancelled orders land here with their receipts."
          />
        </div>
      ) : (
        <DataTable
          columns={columns}
          rows={orders}
          rowKey={(o) => o.id}
          empty={null}
        />
      )}

      <ReceiptDrawer id={receiptId} onClose={() => setReceiptId(null)} />
    </>
  )
}

function ReceiptDrawer({
  id,
  onClose,
}: {
  id: string | null
  onClose: () => void
}) {
  const detail = useQuery({
    ...getMerchantStoreOrderOptions({ path: { id: id ?? '' } }),
    enabled: id != null,
  })
  if (id == null) return null
  const d = detail.data
  const assigned = d && d.status === 'picked_up' ? lookupHandoff(d.id) : null

  return (
    <Drawer
      open
      onClose={onClose}
      title={d ? <>Order <CopyableId id={d.number} /></> : 'Receipt'}
      subtitle={
        d
          ? `${d.store_name} · Placed ${dateTime(d.created_at)} · Cash on delivery`
          : undefined
      }
      status={
        d ? (
          <Status tone={orderStatusTone(d.status)}>
            {orderStatusLabel(d.status)}
          </Status>
        ) : undefined
      }
    >
      {!d ? (
        <div className="flex flex-col gap-3">
          <Skeleton className="h-5 w-40" />
          <Skeleton className="h-24 w-full" />
        </div>
      ) : (
        <>
          {/* The receipt carries everything the live sheet does — same
              endpoint, full record (founder: history must not be poorer). */}
          <DrawerSection label="Customer">
            <div className="text-sm font-semibold">
              {d.customer_name ?? 'Phone verified · name not set'}
            </div>
            <div className="text-[13px] text-text2">{phone(d.customer_phone)}</div>
            <div className="text-[13px] text-text2">{d.address_text}</div>
          </DrawerSection>

          <DrawerSection label={`Items · ${d.store_name}`}>
            {d.items.map((item) => (
              <div
                key={item.product_name}
                className="flex justify-between gap-3 py-1 text-[13.5px]"
              >
                <span>
                  {item.product_name}
                  <span className="ml-1.5 text-xs text-text3">×{item.quantity}</span>
                </span>
                <span>{num(item.unit_price * item.quantity)}</span>
              </div>
            ))}
            <div className="flex justify-between gap-3 py-1 text-[13.5px]">
              <span>Delivery fee</span>
              <span>{num(d.delivery_fee)}</span>
            </div>
            <div className="mt-1.5 flex justify-between border-t border-white/8 pt-3 text-[15px] font-bold">
              <span>Total {d.payment_status === 'collected' ? 'collected' : 'to return'}</span>
              <span className="text-brand">{rwf(d.total)}</span>
            </div>
          </DrawerSection>

          <DrawerSection label="Rider">
            <div className="text-[13px] text-text2">
              {d.status === 'delivered'
                ? assigned != null
                  ? `Delivered by rider #${assigned} — the assignment is frozen.`
                  : 'Delivered — the final rider assignment is frozen.'
                : 'No rider — the order was cancelled before hand-off.'}
            </div>
          </DrawerSection>

          <DrawerSection label="Payment">
            <Status tone={paymentStatusTone(d.payment_status)}>
              {paymentStatusLabel(d.payment_status)}
            </Status>
            {d.status === 'cancelled' ? (
              <div className="text-[13px] text-text2">
                This order was cancelled. Cash taken for it must be returned
                to the customer.
                {d.cancel_reason ? (
                  <div className="mt-1 text-text3">Why: “{d.cancel_reason}”</div>
                ) : null}
              </div>
            ) : null}
          </DrawerSection>
        </>
      )}
    </Drawer>
  )
}
