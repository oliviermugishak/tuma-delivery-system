import { useState } from 'react'

import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Field, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { loginErrorMessage, useLogin } from '@/features/auth/hooks/use-login'

/**
 * Email + password sign-in for merchants and admins. The mutation lives in
 * `use-login.ts`; this component owns only the form state and presentation.
 * Errors render inline via FieldError (role=alert) — persistent and
 * accessible, the right channel for a failed sign-in — while the button
 * reflects isPending.
 */
export function LoginForm() {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const login = useLogin()

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    login.mutate({ body: { email, password } })
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-background px-4">
      <div className="w-full max-w-sm">
        <div className="mb-8 flex flex-col items-center gap-3">
          <img
            src="/brand/android-chrome-192x192.png"
            alt="Tuma"
            className="size-14 rounded-2xl"
          />
          <div className="text-center">
            <h1 className="font-heading text-2xl font-semibold text-foreground">
              Tuma Platform
            </h1>
            <p className="mt-1 text-sm text-muted-foreground">
              Sign in to your account
            </p>
          </div>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Sign in</CardTitle>
            <CardDescription>Merchant and admin access</CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={submit}>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="email">Email</FieldLabel>
                  <Input
                    id="email"
                    type="email"
                    autoComplete="email"
                    required
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    placeholder="you@example.com"
                  />
                </Field>
                <Field>
                  <FieldLabel htmlFor="password">Password</FieldLabel>
                  <Input
                    id="password"
                    type="password"
                    autoComplete="current-password"
                    required
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="••••••••"
                  />
                </Field>

                {login.isError ? (
                  <FieldError>{loginErrorMessage(login.error)}</FieldError>
                ) : null}

                <Button type="submit" disabled={login.isPending} className="w-full">
                  {login.isPending ? 'Signing in…' : 'Sign in'}
                </Button>
              </FieldGroup>
            </form>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
