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
import { Field, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { useUpdateMerchant } from '@/features/admin/hooks/use-update-merchant'

/**
 * "Edit merchant" dialog: name + email. Password resets are deliberately
 * out — an admin who needs to reset one recreates the account. PATCH
 * semantics: only changed fields are sent; an emptied name clears it.
 * Closes on success; on failure (e.g. a taken email) it stays open with
 * the values intact so the admin can fix the input and resubmit.
 */
export function EditMerchantDialog({
  merchant,
  open,
  onOpenChange,
}: {
  merchant: MeResponse | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const updateMerchant = useUpdateMerchant()
  const [name, setName] = useState('')
  const [email, setEmail] = useState('')

  // Seed the form from the merchant each time the dialog opens.
  useEffect(() => {
    if (open && merchant) {
      setName(merchant.name ?? '')
      setEmail(merchant.email ?? '')
    }
  }, [open, merchant])

  if (!merchant) return null

  const emailChanged = email.trim().toLowerCase() !== merchant.email
  const nameChanged = name.trim() !== (merchant.name ?? '')
  const dirty = emailChanged || nameChanged

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!dirty || !email.trim()) return
    const body: Record<string, string> = {}
    if (nameChanged) body.name = name.trim()
    if (emailChanged) body.email = email.trim()
    updateMerchant.mutate(
      { path: { id: merchant.id }, body },
      { onSuccess: () => onOpenChange(false) },
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Edit merchant</DialogTitle>
          <DialogDescription>
            Update the account&apos;s identity. Clearing the name removes it.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="edit-merchant-name">Name</FieldLabel>
              <Input
                id="edit-merchant-name"
                autoComplete="off"
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Acme Café"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="edit-merchant-email">Email</FieldLabel>
              <Input
                id="edit-merchant-email"
                type="email"
                autoComplete="off"
                required
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="owner@acme.rw"
              />
            </Field>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={updateMerchant.isPending}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={!dirty || !email.trim() || updateMerchant.isPending}
              >
                {updateMerchant.isPending ? 'Saving…' : 'Save changes'}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}
