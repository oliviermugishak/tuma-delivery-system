/**
 * Sign-in (shared by both wings — P16). Email + password; the mutation
 * lives in use-login (session cookies are set by the server; the hook
 * seeds the cache and routes to the wing's home). Inline validation,
 * one accent button, no shadows (P6), designed error state.
 */
import { useState } from 'react'

import { Button, Field, Icon, Input } from '@/components/ds'
import { loginErrorMessage, useLogin } from '../hooks/use-login'

export function LoginForm() {
  const login = useLogin()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [touched, setTouched] = useState(false)

  const emailInvalid = touched && !/^\S+@\S+\.\S+$/.test(email)
  const passwordInvalid = touched && password.length === 0
  const invalid = emailInvalid || passwordInvalid

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    setTouched(true)
    if (invalid) return
    login.mutate({ body: { email, password } })
  }

  return (
    <div className="grid min-h-screen place-items-center bg-background px-4">
      <div className="w-full max-w-100">
        <div className="mb-8 flex flex-col items-center gap-3">
          <img
            src="/brand/android-chrome-192x192.png"
            alt="Tuma"
            className="size-14 rounded-2xl"
          />
          <div className="text-center">
            <h1 className="text-[22px] font-extrabold tracking-tight">
              Tuma Platform
            </h1>
            <p className="mt-1 text-sm text-text2">
              Sign in to run your business — or the platform.
            </p>
          </div>
        </div>

        <form
          onSubmit={submit}
          className="flex flex-col gap-4 rounded-2xl bg-surface p-6"
        >
          <Field
            label="Email"
            error={emailInvalid ? 'Enter your email address.' : undefined}
          >
            <Input
              type="email"
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              onBlur={() => setTouched(true)}
              placeholder="you@example.com"
              invalid={emailInvalid}
            />
          </Field>
          <Field
            label="Password"
            error={passwordInvalid ? 'Enter your password.' : undefined}
          >
            <Input
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="••••••••"
              invalid={passwordInvalid}
            />
          </Field>

          {login.isError ? (
            <div role="alert" className="text-xs font-medium text-danger">
              {loginErrorMessage(login.error)}
            </div>
          ) : null}

          <Button
            type="submit"
            variant="primary"
            className="mt-1 w-full"
            disabled={login.isPending}
          >
            {login.isPending ? (
              'Signing in…'
            ) : (
              <>
                Sign in
                <Icon name="arrow_forward" label="" size={18} />
              </>
            )}
          </Button>
        </form>

        <p className="mt-6 text-center text-xs text-text3">
          Customers order in the Tuma app — this platform is for merchants,
          riders, and the Tuma team.
        </p>
      </div>
    </div>
  )
}
