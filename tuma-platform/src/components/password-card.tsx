import { useState } from 'react'

import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { useChangePassword } from '@/hooks/use-change-password'

/**
 * Change-password card (POST /v1/auth/password). The confirm field is
 * checked client-side (inline FieldError); everything else is the server's
 * call — a wrong current password comes back as a 401 whose message the
 * hook toasts, and the session survives it (the 401 watcher skips this
 * endpoint).
 */
export function PasswordCard() {
  const changePassword = useChangePassword()
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')

  const mismatch =
    confirmPassword.length > 0 && confirmPassword !== newPassword

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    if (newPassword !== confirmPassword) return
    changePassword.mutate(
      { body: { current_password: currentPassword, new_password: newPassword } },
      {
        onSuccess: () => {
          setCurrentPassword('')
          setNewPassword('')
          setConfirmPassword('')
        },
      },
    )
  }

  return (
    <Card className="overflow-hidden">
      <CardHeader className="border-b bg-muted/20">
        <CardTitle>Password</CardTitle>
        <CardDescription>
          Change the password you use to sign in.
        </CardDescription>
      </CardHeader>
      <CardContent className="p-5">
        <form onSubmit={submit}>
          <FieldGroup className="max-w-md">
            <Field>
              <FieldLabel htmlFor="current-password">
                Current password
              </FieldLabel>
              <Input
                id="current-password"
                type="password"
                autoComplete="current-password"
                required
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
                placeholder="••••••••"
              />
            </Field>
            <div className="grid gap-4 sm:grid-cols-2">
              <Field>
                <FieldLabel htmlFor="new-password">New password</FieldLabel>
                <Input
                  id="new-password"
                  type="password"
                  autoComplete="new-password"
                  required
                  minLength={8}
                  maxLength={128}
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                  placeholder="••••••••"
                />
                <FieldDescription>At least 8 characters.</FieldDescription>
              </Field>
              <Field data-invalid={mismatch || undefined}>
                <FieldLabel htmlFor="confirm-password">
                  Confirm new password
                </FieldLabel>
                <Input
                  id="confirm-password"
                  type="password"
                  autoComplete="new-password"
                  required
                  minLength={8}
                  maxLength={128}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  placeholder="••••••••"
                />
                {mismatch ? (
                  <FieldError>The passwords do not match.</FieldError>
                ) : null}
              </Field>
            </div>
            <Button
              type="submit"
              disabled={changePassword.isPending || mismatch}
              className="w-fit"
            >
              {changePassword.isPending ? 'Changing…' : 'Change password'}
            </Button>
          </FieldGroup>
        </form>
      </CardContent>
    </Card>
  )
}
