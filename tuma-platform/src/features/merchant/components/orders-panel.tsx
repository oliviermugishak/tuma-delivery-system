import { useState } from 'react'
import {
  Ban,
  Check,
  ChevronRight,
  CookingPot,
  PackageCheck,
  RefreshCw,
  ShoppingBag,
} from 'lucide-react'

import type { MerchantStoreOrderResponse } from '@/api/generated'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { HandoffDialog } from '@/features/merchant/components/handoff-dialog'
import { OrderDetailDialog, StatusBadge } from '@/features/merchant/components/order-detail-dialog'
import {
  useAdvanceStoreOrder,
} from '@/features/merchant/hooks/use-advance-store-order'
import { useMerchantOrders } from '@/features/merchant/hooks/use-merchant-orders'
import { formatDate, formatRwf } from '@/lib/format'

/**
 * The operating board: every incoming store order across the operator's
 * authorized stores, newest first, polling gently so new work shows up on
 * its own. Each row advances along placed → accepted → preparing →
 * picked_up → delivered — or rejects with cancel while the order is still
 * on the premises. Store-scoped members see only their store. The
 * preparing → picked_up step is the REAL handoff: the operator types the
 * rider's number (the server assigns them, caches the route, stamps the
 * handoff) — a bare advance is refused by the server, by design.
 */
export function OrdersPanel() {
  const orders = useMerchantOrders()
  const advance = useAdvanceStoreOrder()
  const [selected, setSelected] = useState<MerchantStoreOrderResponse | null>(
    null,
  )
  const [handoffOrder, setHandoffOrder] =
    useState<MerchantStoreOrderResponse | null>(null)

  // The id whose transition is in flight — that row's buttons disable
  // until the server answers.
  const pendingId = advance.isPending
    ? (advance.variables?.path?.id ?? null)
    : null

  return (
    <div>
      <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="font-heading text-2xl font-semibold tracking-tight">
            Orders
          </h2>
          <p className="mt-1.5 max-w-xl text-sm text-muted-foreground">
            Incoming store orders across your business. Accept, prepare, and
            hand off — each store fulfills its own orders.
          </p>
        </div>
        <Button
          variant="outline"
          onClick={() => void orders.refetch()}
          disabled={orders.isFetching}
        >
          <RefreshCw data-icon="inline-start" />
          Refresh
        </Button>
      </div>

      {orders.isLoading ? (
        <div className="flex items-center justify-center py-24">
          <div
            aria-label="Loading orders"
            className="size-7 animate-spin rounded-full border-2 border-primary border-t-transparent"
          />
        </div>
      ) : null}

      {orders.isError ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">Couldn&apos;t load your orders</p>
            <p className="text-sm text-muted-foreground">
              Something went wrong while fetching the board.
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void orders.refetch()}
            >
              <RefreshCw data-icon="inline-start" />
              Try again
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {orders.isSuccess && orders.data.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-4 py-20 text-center">
            <div className="flex size-14 items-center justify-center rounded-2xl bg-primary/10">
              <ShoppingBag className="size-7 text-primary" aria-hidden />
            </div>
            <div className="grid gap-1">
              <p className="font-heading text-lg font-semibold">
                No orders yet
              </p>
              <p className="mx-auto max-w-sm text-sm text-muted-foreground">
                When customers check out, their store orders appear here —
                accept them to start cooking.
              </p>
            </div>
          </CardContent>
        </Card>
      ) : null}

      {orders.isSuccess && orders.data.length > 0 ? (
        <Card>
          <CardContent className="py-2">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Order</TableHead>
                  <TableHead>Store</TableHead>
                  <TableHead>Deliver to</TableHead>
                  <TableHead>Total</TableHead>
                  <TableHead>Placed</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {orders.data.map((order) => (
                  <StoreOrderRow
                    key={order.id}
                    order={order}
                    pending={pendingId === order.id}
                    onAdvance={(status) =>
                      advance.mutate({
                        path: { id: order.id },
                        body: { status },
                      })
                    }
                    onHandoff={() => setHandoffOrder(order)}
                    onOpen={() => setSelected(order)}
                  />
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      ) : null}

      <OrderDetailDialog
        order={selected}
        open={selected !== null}
        onOpenChange={(open) => {
          if (!open) setSelected(null)
        }}
      />
      <HandoffDialog
        order={handoffOrder}
        open={handoffOrder !== null}
        onOpenChange={(open) => {
          if (!open) setHandoffOrder(null)
        }}
      />
    </div>
  )
}

function StoreOrderRow({
  order,
  pending,
  onAdvance,
  onHandoff,
  onOpen,
}: {
  order: MerchantStoreOrderResponse
  pending: boolean
  onAdvance: (status: string) => void
  onHandoff: () => void
  onOpen: () => void
}) {
  return (
    <TableRow
      className="cursor-pointer"
      onClick={onOpen}
    >
      <TableCell className="font-medium tabular-nums">
        #{order.number}
      </TableCell>
      <TableCell>{order.store_name}</TableCell>
      <TableCell className="max-w-56">
        <span className="line-clamp-1 text-muted-foreground" title={order.address_text}>
          {order.address_text}
        </span>
      </TableCell>
      <TableCell className="font-semibold">{formatRwf(order.total)}</TableCell>
      <TableCell className="text-muted-foreground">
        {formatDate(order.created_at)}
      </TableCell>
      <TableCell>
        <StatusBadge status={order.status} />
      </TableCell>
      <TableCell className="text-right">
        <div
          className="flex items-center justify-end gap-1"
          onClick={(event) => event.stopPropagation()}
        >
          <NextActions
            status={order.status}
            pending={pending}
            onAdvance={onAdvance}
            onHandoff={onHandoff}
          />
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={onOpen}
            aria-label={`Open order ${order.number}`}
          >
            <ChevronRight aria-hidden />
          </Button>
        </div>
      </TableCell>
    </TableRow>
  )
}

/** The one legitimate next step (plus reject while on the premises). */
function NextActions({
  status,
  pending,
  onAdvance,
  onHandoff,
}: {
  status: string
  pending: boolean
  onAdvance: (status: string) => void
  onHandoff: () => void
}) {
  // preparing's next step is the handoff DIALOG (the rider's number is
  // required — the server refuses a bare picked_up advance), so it gets
  // no `status` here; everything else is a plain advance.
  const next: { status: string; label: string; icon: typeof Check } | null =
    status === 'placed'
      ? { status: 'accepted', label: 'Accept', icon: Check }
      : status === 'accepted'
        ? { status: 'preparing', label: 'Prepare', icon: CookingPot }
        : status === 'picked_up'
          ? { status: 'delivered', label: 'Delivered', icon: PackageCheck }
          : null

  return (
    <div className="flex items-center justify-end gap-1">
      {next ? (
        <Button
          size="sm"
          variant="outline"
          disabled={pending}
          onClick={() => onAdvance(next.status)}
        >
          <next.icon data-icon="inline-start" />
          {pending ? '…' : next.label}
        </Button>
      ) : null}
      {status === 'preparing' ? (
        <Button
          size="sm"
          variant="outline"
          disabled={pending}
          onClick={onHandoff}
        >
          <ShoppingBag data-icon="inline-start" />
          Handed to rider
        </Button>
      ) : null}
      {status === 'placed' || status === 'accepted' || status === 'preparing' ? (
        <Button
          size="sm"
          variant="ghost"
          disabled={pending}
          className="text-destructive hover:bg-destructive/10 hover:text-destructive"
          onClick={() => onAdvance('cancelled')}
        >
          <Ban data-icon="inline-start" />
          Reject
        </Button>
      ) : null}
      {next === null &&
      (status === 'delivered' || status === 'cancelled') ? (
        <ChevronRight className="size-4 text-muted-foreground" aria-hidden />
      ) : null}
    </div>
  )
}
