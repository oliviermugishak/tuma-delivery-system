import { useEffect, useState } from 'react'

import { Bike } from 'lucide-react'

import type { MerchantStoreOrderResponse } from '@/api/generated'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Field,
  FieldDescription,
  FieldLabel,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { useHandoffStoreOrder } from '@/features/merchant/hooks/use-handoff-store-order'

/**
 * The real "Handed to rider" (tracking doc §5): the operator types the
 * rider's memorized number — that is the whole interface, no directory —
 * and the server assigns the rider, stamps the handoff, and caches the
 * route. The dialog stays open with the value intact on failure (a wrong
 * number is a server 404 naming itself) so it can be corrected and
 * resubmitted. Re-assignment of an out-for-delivery order reuses the
 * same action until the delivery is done.
 */
export function HandoffDialog({
  order,
  open,
  onOpenChange,
  reassign = false,
}: {
  order: MerchantStoreOrderResponse | null
  open: boolean
  onOpenChange: (open: boolean) => void
  reassign?: boolean
}) {
  const handoff = useHandoffStoreOrder()
  const [riderNumber, setRiderNumber] = useState('')

  // Fresh form every time the dialog opens.
  useEffect(() => {
    if (open) setRiderNumber('')
  }, [open])

  if (!order) return null
  const pending = handoff.isPending

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    const number = Number(riderNumber)
    if (!Number.isInteger(number) || number <= 0) return
    handoff.mutate(
      {
        path: { id: order.id },
        body: { rider_number: number },
      },
      { onSuccess: () => onOpenChange(false) },
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Bike className="size-5 text-primary" aria-hidden />
            {reassign ? 'Re-assign a rider' : 'Hand to rider'}
          </DialogTitle>
          <DialogDescription>
            Type the rider's number — the one shown on their app. The order
            moves to out-for-delivery, the route is cached, and the rider's
            phone gets the job.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <Field>
            <FieldLabel htmlFor="rider-number">Rider number</FieldLabel>
            <Input
              id="rider-number"
              inputMode="numeric"
              autoComplete="off"
              autoFocus
              required
              value={riderNumber}
              onChange={(e) =>
                setRiderNumber(e.target.value.replace(/\D/g, ''))
              }
              placeholder="e.g. 12"
            />
            <FieldDescription>
              Only an active rider's number is accepted — unknown or
              deactivated riders are rejected.
            </FieldDescription>
          </Field>
          <DialogFooter className="mt-4">
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={pending}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={pending || riderNumber === ''}>
              {pending
                ? 'Handing over…'
                : reassign
                  ? 'Re-assign rider'
                  : 'Hand to rider'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
