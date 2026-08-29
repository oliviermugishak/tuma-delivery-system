import { useEffect, useState } from 'react'

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
import { useCreateMerchant } from '@/features/admin/hooks/use-create-merchant'

/**
 * "Add merchant" dialog — provisions a BUSINESS, not a login: the business
 * row, the owner's account (email + password), and the owner membership in
 * one call. Controlled form structured with shadcn Field primitives (same
 * pattern as the login form) — no form library until a real need earns
 * one. The mutation (toast + list invalidation) lives in
 * use-create-merchant; this component owns the field state and closes
 * itself on success. On failure the dialog stays open with the values
 * intact so the admin can fix the input (e.g. a taken owner email — the
 * server message names it) and resubmit.
 */
export function CreateMerchantDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const createMerchant = useCreateMerchant()
  const [name, setName] = useState('')
  const [businessEmail, setBusinessEmail] = useState('')
  const [ownerEmail, setOwnerEmail] = useState('')
  const [password, setPassword] = useState('')

  // Fresh form every time the dialog opens.
  useEffect(() => {
    if (open) {
      setName('')
      setBusinessEmail('')
      setOwnerEmail('')
      setPassword('')
    }
  }, [open])

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    createMerchant.mutate(
      {
        body: {
          name: name.trim(),
          business_email: businessEmail.trim() || null,
          email: ownerEmail.trim(),
          password,
        },
      },
      { onSuccess: () => onOpenChange(false) },
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add merchant</DialogTitle>
          <DialogDescription>
            Creates the business and its owner account. The owner signs into
            the merchant wing with the email and password.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="merchant-name">Business name</FieldLabel>
              <Input
                id="merchant-name"
                autoComplete="off"
                required
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Acme Café"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="merchant-business-email">
                Business email (optional)
              </FieldLabel>
              <Input
                id="merchant-business-email"
                type="email"
                autoComplete="off"
                value={businessEmail}
                onChange={(e) => setBusinessEmail(e.target.value)}
                placeholder="hello@acme.rw"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="merchant-owner-email">
                Owner email
              </FieldLabel>
              <Input
                id="merchant-owner-email"
                type="email"
                autoComplete="off"
                required
                value={ownerEmail}
                onChange={(e) => setOwnerEmail(e.target.value)}
                placeholder="owner@acme.rw"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="merchant-owner-password">
                Owner password
              </FieldLabel>
              <Input
                id="merchant-owner-password"
                type="password"
                autoComplete="new-password"
                required
                minLength={8}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="••••••••"
              />
            </Field>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={createMerchant.isPending}
              >
                Cancel
              </Button>
              <Button type="submit" disabled={createMerchant.isPending}>
                {createMerchant.isPending ? 'Creating…' : 'Create merchant'}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}
