import { useEffect, useState } from 'react'

import type { MeResponse } from '@/api/generated'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Field, FieldDescription, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { useUpdateCustomer } from '@/features/admin/hooks/use-update-customer'

/**
 * "Edit customer" dialog: name + phone. Phone is a customer's identity —
 * editing it is how an admin fixes a typo'd number from OTP signup (the
 * server rejects a phone another account already holds with a 409).
 * PATCH semantics: only changed fields are sent; an emptied name clears
 * it. Closes on success; stays open on failure with values intact.
 */
export function EditCustomerDialog({
  customer,
  open,
  onOpenChange,
}: {
  customer: MeResponse | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const updateCustomer = useUpdateCustomer()
  const [name, setName] = useState('')
  const [phone, setPhone] = useState('')

  // Seed the form from the customer each time the dialog opens.
  useEffect(() => {
    if (open && customer) {
      setName(customer.name ?? '')
      setPhone(customer.phone ?? '')
    }
  }, [open, customer])

  if (!customer) return null

  const phoneChanged = phone.trim() !== customer.phone
  const nameChanged = name.trim() !== (customer.name ?? '')
  const dirty = phoneChanged || nameChanged

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!dirty || !phone.trim()) return
    const body: Record<string, string> = {}
    if (nameChanged) body.name = name.trim()
    if (phoneChanged) body.phone = phone.trim()
    updateCustomer.mutate(
      { path: { id: customer.id }, body },
      { onSuccess: () => onOpenChange(false) },
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Edit customer</DialogTitle>
          <DialogDescription>
            Update the account&apos;s identity. Clearing the name removes it.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="edit-customer-name">Name</FieldLabel>
              <Input
                id="edit-customer-name"
                autoComplete="off"
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Aline"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="edit-customer-phone">Phone</FieldLabel>
              <Input
                id="edit-customer-phone"
                type="tel"
                autoComplete="off"
                required
                value={phone}
                onChange={(e) => setPhone(e.target.value)}
                placeholder="+250 7XX XXX XXX"
              />
              <FieldDescription>
                The number they verify with in the app.
              </FieldDescription>
            </Field>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={updateCustomer.isPending}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={!dirty || !phone.trim() || updateCustomer.isPending}
              >
                {updateCustomer.isPending ? 'Saving…' : 'Save changes'}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}
