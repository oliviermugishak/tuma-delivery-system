/**
 * Settings (coverage §7.17) — shared by both wings (P16: role is a
 * mode). Identity appears exactly once (P1); the name field starts
 * empty with its helper, never prefilled with the email; the single
 * primary action saves the profile; password change is the outline
 * secondary. Dirty-save bar while edits exist.
 *
 * Merchant operators: the server gives their account no editable profile
 * name in V1 — the page says so honestly instead of showing a dead form
 * (P2/P3), and offers only the password change.
 */
import { useState } from 'react'

import { Card, Field, Input, PageHead, Button, DirtySaveBar } from '@/components/ds'
import { useChangePassword } from '@/hooks/use-change-password'
import { useUpdateMe } from '@/hooks/use-update-me'
import { useSession } from '@/hooks/use-session'

export function SettingsScreen() {
  const { data: user } = useSession()
  const isMerchantOperator = (user?.merchant_memberships.length ?? 0) > 0 && !user?.admin

  return (
    <div className="flex flex-col gap-5">
      <PageHead title="Settings" />
      {isMerchantOperator ? (
        <>
          <Card>
            <div className="text-[17px] font-bold">Identity</div>
            <p className="mt-1 text-sm text-text2">
              You sign in as {user?.email}. A merchant operator's identity on
              Tuma is the business — there's no separate display name to edit.
            </p>
          </Card>
          <PasswordCard />
        </>
      ) : (
        <ProfileCard email={user?.email ?? ''} />
      )}
    </div>
  )
}

function ProfileCard({ email }: { email: string }) {
  const { data: user } = useSession()
  const updateMe = useUpdateMe()
  const savedName = user?.admin?.name || user?.customer?.name || ''
  const [name, setName] = useState('')
  const [touched, setTouched] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const value = touched ? name : savedName

  const save = () => {
    const trimmed = value.trim()
    if (!trimmed) {
      setError('Give the account a name to save.')
      return
    }
    setError(null)
    updateMe.mutate(
      { body: { name: trimmed } },
      { onSuccess: () => setTouched(false) },
    )
  }

  return (
    <div className="relative">
      <Card>
        <div className="text-[17px] font-bold">Identity</div>
        <div className="mt-4 max-w-150">
          <Field
            label="Name"
            help={`Shown to your team · signed in as ${email}`}
            error={error}
          >
            <Input
              value={value}
              onChange={(e) => {
                setTouched(true)
                setName(e.target.value)
              }}
              placeholder={savedName || 'Your name'}
            />
          </Field>
        </div>
      </Card>
      <DirtySaveBar
        open={touched && value !== savedName}
        what="Identity"
        onSave={save}
        onDiscard={() => {
          setTouched(false)
          setError(null)
        }}
        saving={updateMe.isPending}
      />
      <div className="mt-5">
        <PasswordCard />
      </div>
    </div>
  )
}

function PasswordCard() {
  const changePassword = useChangePassword()
  const [current, setCurrent] = useState('')
  const [next, setNext] = useState('')
  const [confirm, setConfirm] = useState('')
  const [error, setError] = useState<string | null>(null)

  const submit = () => {
    if (!current || !next) {
      setError('Fill in both the current and the new password.')
      return
    }
    if (next !== confirm) {
      setError('The new passwords don’t match.')
      return
    }
    if (next.length < 8) {
      setError('The new password must be at least 8 characters.')
      return
    }
    setError(null)
    changePassword.mutate(
      { body: { current_password: current, new_password: next } },
      {
        onSuccess: () => {
          setCurrent('')
          setNext('')
          setConfirm('')
        },
        onError: (e) => setError(e.message),
      },
    )
  }

  return (
    <Card>
      <div className="text-[17px] font-bold">Password</div>
      <div className="mt-4 grid max-w-150 gap-4">
        <Field label="Current password">
          <Input
            type="password"
            autoComplete="current-password"
            value={current}
            onChange={(e) => setCurrent(e.target.value)}
          />
        </Field>
        <div className="grid gap-4 sm:grid-cols-2">
          <Field label="New password" help="At least 8 characters">
            <Input
              type="password"
              autoComplete="new-password"
              value={next}
              onChange={(e) => setNext(e.target.value)}
            />
          </Field>
          <Field label="Repeat new password">
            <Input
              type="password"
              autoComplete="new-password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
            />
          </Field>
        </div>
        {error ? (
          <div className="text-xs font-medium text-danger" role="alert">
            {error}
          </div>
        ) : null}
        <div>
          <Button variant="outline" onClick={submit} disabled={changePassword.isPending}>
            {changePassword.isPending ? 'Changing…' : 'Change password'}
          </Button>
        </div>
      </div>
    </Card>
  )
}
