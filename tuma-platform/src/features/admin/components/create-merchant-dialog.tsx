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
 * "Add merchant" dialog. Controlled form structured with shadcn Field
 * primitives (same pattern as the login form) — no form library until a
 * real need earns one. Merchants are email + password identity; phone
 * numbers are customer identity only, so there is no phone field. The
 * mutation (toast + list invalidation) lives in use-create-merchant; this
 * component owns the field state and closes itself on success. On failure
 * the dialog stays open with the values intact so the admin can fix the
 * input (e.g. a taken email — the server message names it) and resubmit.
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
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')

  // Fresh form every time the dialog opens.
  useEffect(() => {
    if (open) {
      setName('')
      setEmail('')
      setPassword('')
    }
  }, [open])

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    createMerchant.mutate(
      {
        body: {
          name: name.trim() || null,
          email: email.trim(),
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
            Create a merchant account. They sign in with this email and
            password.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="merchant-name">Name</FieldLabel>
              <Input
                id="merchant-name"
                autoComplete="off"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Acme Café"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="merchant-email">Email</FieldLabel>
              <Input
                id="merchant-email"
                type="email"
                autoComplete="off"
                required
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="owner@acme.rw"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="merchant-password">Password</FieldLabel>
              <Input
                id="merchant-password"
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
