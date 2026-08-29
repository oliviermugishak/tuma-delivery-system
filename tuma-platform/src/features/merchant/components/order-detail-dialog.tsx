import { ExternalLink, Phone, UserRound } from 'lucide-react'

import type { MerchantStoreOrderResponse } from '@/api/generated'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Skeleton } from '@/components/ui/skeleton'
import {
  useAdvanceStoreOrder,
} from '@/features/merchant/hooks/use-advance-store-order'
import {
  useMerchantStoreOrder,
} from '@/features/merchant/hooks/use-merchant-store-order'
import { formatDate, formatRwf } from '@/lib/format'

/**
 * The fulfillment sheet for one store order, opened from the board:
 * what to prepare (items), where it goes (address + coordinates), who to
 * call (customer name + phone — local reality is that someone calls when
 * the rider is at the gate), and the money. The advance/reject actions
 * live here too, so the operator never leaves the dialog to work an order.
 */
export function OrderDetailDialog({
  order,
  open,
  onOpenChange,
}: {
  order: MerchantStoreOrderResponse | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const detail = useMerchantStoreOrder(open && order ? order.id : '')
  const advance = useAdvanceStoreOrder()

  const pendingId = advance.isPending
    ? (advance.variables?.path?.id ?? null)
    : null
  const pending = pendingId === order?.id

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
        {detail.isLoading || !order ? (
          <div className="grid gap-3 p-2">
            <Skeleton className="h-7 w-40" />
            <Skeleton className="h-24 rounded-xl" />
            <Skeleton className="h-40 rounded-xl" />
          </div>
        ) : detail.isError ? (
          <DialogHeader>
            <DialogTitle>Order #{order.number}</DialogTitle>
            <DialogDescription>
              Couldn&apos;t load the detail — try again from the board.
            </DialogDescription>
          </DialogHeader>
        ) : detail.data ? (
          <>
            <DialogHeader>
              <DialogTitle className="flex items-center gap-2">
                Order #{detail.data.number}
                <StatusBadge status={detail.data.status} />
              </DialogTitle>
              <DialogDescription>
                {detail.data.store_name} · placed{' '}
                {formatDate(detail.data.created_at)}
              </DialogDescription>
            </DialogHeader>

            <div className="grid gap-4">
              {/* The customer contact — why they call. */}
              <section className="rounded-xl border bg-muted/20 p-4">
                <p className="flex items-center gap-2 text-sm font-semibold">
                  <UserRound className="size-4 text-primary" aria-hidden />
                  Customer
                </p>
                <p className="mt-2 text-sm">
                  {detail.data.customer_name || 'Name not given'}
                </p>
                {detail.data.customer_phone ? (
                  <a
                    href={`tel:${detail.data.customer_phone}`}
                    className="mt-1 inline-flex items-center gap-1.5 text-sm text-primary hover:underline"
                  >
                    <Phone className="size-3.5" aria-hidden />
                    {detail.data.customer_phone}
                  </a>
                ) : (
                  <p className="mt-1 text-sm text-muted-foreground">
                    No phone on the account.
                  </p>
                )}
                <p className="mt-2 text-sm">{detail.data.address_text}</p>
                {detail.data.address_lat != null &&
                detail.data.address_lng != null ? (
                  <a
                    href={`https://www.openstreetmap.org/?mlat=${detail.data.address_lat}&mlon=${detail.data.address_lng}#map=17/${detail.data.address_lat}/${detail.data.address_lng}`}
                    target="_blank"
                    rel="noreferrer"
                    className="mt-1 inline-flex items-center gap-1.5 text-xs text-primary hover:underline"
                  >
                    <ExternalLink className="size-3" aria-hidden />
                    Open the delivery pin on the map
                  </a>
                ) : null}
              </section>

              {/* What to prepare. */}
              <section>
                <p className="mb-2 text-sm font-semibold">
                  Items ({detail.data.items.length})
                </p>
                <div className="divide-y rounded-xl border">
                  {detail.data.items.map((item, index) => (
                    <div
                      key={index}
                      className="flex items-center justify-between gap-3 px-4 py-2.5"
                    >
                      <span className="text-sm">
                        <span className="font-semibold">{item.quantity}×</span>{' '}
                        {item.product_name}
                      </span>
                      <span className="text-sm font-medium">
                        {formatRwf(item.unit_price * item.quantity)}
                      </span>
                    </div>
                  ))}
                  <div className="flex items-center justify-between px-4 py-2.5 text-sm text-muted-foreground">
                    <span>Delivery</span>
                    <span>{formatRwf(detail.data.delivery_fee)}</span>
                  </div>
                  <div className="flex items-center justify-between px-4 py-2.5">
                    <span className="text-sm font-semibold">Total</span>
                    <span className="font-semibold">
                      {formatRwf(detail.data.total)}
                    </span>
                  </div>
                </div>
                <p className="mt-2 text-xs text-muted-foreground">
                  Payment: {detail.data.payment_status.replaceAll('_', ' ')}
                  {detail.data.payment_status === 'pending'
                    ? ' — collect cash on delivery'
                    : ''}
                </p>
              </section>

              {/* Actions mirror the board so the operator never leaves. */}
              <div className="flex flex-wrap items-center justify-end gap-2 border-t pt-4">
                <NextActions
                  status={detail.data.status}
                  pending={pending}
                  onAdvance={(status) =>
                    advance.mutate({
                      path: { id: detail.data.id },
                      body: { status },
                    })
                  }
                />
              </div>
            </div>
          </>
        ) : null}
      </DialogContent>
    </Dialog>
  )
}

export function StatusBadge({ status }: { status: string }) {
  const styles: Record<string, string> = {
    placed: 'bg-primary/10 text-primary',
    accepted: 'bg-sky-500/10 text-sky-500',
    preparing: 'bg-sky-500/10 text-sky-500',
    picked_up: 'bg-purple-500/10 text-purple-500',
    delivered: 'bg-emerald-500/10 text-emerald-600',
    cancelled: 'bg-destructive/10 text-destructive',
  }
  const labels: Record<string, string> = {
    placed: 'Placed',
    accepted: 'Accepted',
    preparing: 'Preparing',
    picked_up: 'Out for delivery',
    delivered: 'Delivered',
    cancelled: 'Cancelled',
  }
  return (
    <span
      className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold ${styles[status] ?? 'bg-muted text-muted-foreground'}`}
    >
      {labels[status] ?? status}
    </span>
  )
}

function NextActions({
  status,
  pending,
  onAdvance,
}: {
  status: string
  pending: boolean
  onAdvance: (status: string) => void
}) {
  const next =
    status === 'placed'
      ? { status: 'accepted', label: 'Accept' }
      : status === 'accepted'
        ? { status: 'preparing', label: 'Start preparing' }
        : status === 'preparing'
          ? { status: 'picked_up', label: 'Handed to rider' }
          : status === 'picked_up'
            ? { status: 'delivered', label: 'Delivered' }
            : null

  return (
    <>
      {next ? (
        <Button
          size="sm"
          disabled={pending}
          onClick={() => onAdvance(next.status)}
        >
          {pending ? 'Saving…' : next.label}
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
          Reject
        </Button>
      ) : null}
    </>
  )
}
