import { useEffect, useState } from 'react'

import type { MerchantResponse } from '@/api/generated'
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
  merchant: MerchantResponse | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const updateMerchant = useUpdateMerchant()
  const [name, setName] = useState('')
  const [businessEmail, setBusinessEmail] = useState('')
  const [businessPhone, setBusinessPhone] = useState('')

  // Seed the form from the business each time the dialog opens.
  useEffect(() => {
    if (open && merchant) {
      setName(merchant.name)
      setBusinessEmail(merchant.business_email ?? '')
      setBusinessPhone(merchant.business_phone ?? '')
    }
  }, [open, merchant])

  if (!merchant) return null

  const nameChanged = name.trim() !== merchant.name
  const businessEmailChanged =
    businessEmail.trim() !== (merchant.business_email ?? '')
  const businessPhoneChanged =
    businessPhone.trim() !== (merchant.business_phone ?? '')
  const dirty = nameChanged || businessEmailChanged || businessPhoneChanged

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!dirty || !name.trim()) return
    const body: {
      name?: string
      business_email?: string | null
      business_phone?: string | null
      status: string
    } = {
      // The generated PATCH carries status as a required field — echo the
      // current one so an identity edit never accidentally suspends.
      status: merchant.status,
    }
    if (nameChanged) body.name = name.trim()
    if (businessEmailChanged) {
      body.business_email = businessEmail.trim() || null
    }
    if (businessPhoneChanged) {
      body.business_phone = businessPhone.trim() || null
    }
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
            The business&apos;s identity and contact details. Owner sign-ins
            are managed per account, not here.
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
              <FieldLabel htmlFor="edit-merchant-business-email">
                Business email
              </FieldLabel>
              <Input
                id="edit-merchant-business-email"
                type="email"
                autoComplete="off"
                value={businessEmail}
                onChange={(e) => setBusinessEmail(e.target.value)}
                placeholder="hello@acme.rw"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="edit-merchant-business-phone">
                Business phone
              </FieldLabel>
              <Input
                id="edit-merchant-business-phone"
                autoComplete="off"
                value={businessPhone}
                onChange={(e) => setBusinessPhone(e.target.value)}
                placeholder="+250 788 000 000"
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
                disabled={!dirty || !name.trim() || updateMerchant.isPending}
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
