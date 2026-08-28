import { useEffect, useState } from 'react'

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
  FieldGroup,
  FieldLabel,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { useUpdateMe } from '@/hooks/use-update-me'
import { useSession } from '@/hooks/use-session'
import { formatDate, initials } from '@/lib/format'
import { Avatar, AvatarFallback } from './ui/avatar'

/**
 * Profile card: the account's identity (avatar, name, email, role), its
 * read-only facts, and the one editable field in V1 — the display name
 * (PATCH /v1/me). The email is set by the admin when the account is
 * created; self-service email changes are not a V1 thing.
 */
export function ProfileCard() {
  const { data: user } = useSession()
  const updateMe = useUpdateMe()
  const [name, setName] = useState(user?.name ?? '')

  // The session is in the cache before this renders, but stay in sync if
  // /me data changes elsewhere.
  useEffect(() => {
    setName(user?.name ?? '')
  }, [user?.name])

  const dirty = name.trim() !== (user?.name ?? '')

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    updateMe.mutate({ body: { name: name.trim() } })
  }

  return (
    <Card className="overflow-hidden">
      <CardHeader className="border-b bg-muted/20">
        <CardTitle>Profile</CardTitle>
        <CardDescription>Your account details on Tuma.</CardDescription>
      </CardHeader>
      <CardContent className="space-y-6 p-5">
        <div className="flex items-center gap-4">
          <Avatar size="lg">
            <AvatarFallback className="bg-primary/10 font-semibold text-primary">
              {initials(user?.name, user?.email)}
            </AvatarFallback>
          </Avatar>
          <div className="grid gap-0.5">
            <p className="text-sm font-medium">
              {user?.name || user?.email || 'Signed in'}
            </p>
            <p className="text-sm text-muted-foreground">{user?.email}</p>
            <p className="text-xs text-muted-foreground capitalize">
              {user?.role}
            </p>
          </div>
        </div>

        <dl className="grid gap-3 rounded-lg border bg-muted/20 p-4 text-sm sm:grid-cols-2">
          <div>
            <dt className="text-xs font-medium tracking-wider text-muted-foreground uppercase">
              Email
            </dt>
            <dd className="mt-1 truncate font-medium">{user?.email || '—'}</dd>
          </div>
          <div>
            <dt className="text-xs font-medium tracking-wider text-muted-foreground uppercase">
              Member since
            </dt>
            <dd className="mt-1 font-medium">
              {formatDate(user?.created_at)}
            </dd>
          </div>
        </dl>

        <form onSubmit={submit}>
          <FieldGroup className="max-w-md">
            <Field>
              <FieldLabel htmlFor="profile-name">Name</FieldLabel>
              <Input
                id="profile-name"
                autoComplete="name"
                required
                minLength={1}
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Your name"
              />
              <FieldDescription>
                Shown to customers on your store and orders.
              </FieldDescription>
            </Field>
            <Button
              type="submit"
              disabled={updateMe.isPending || !dirty}
              className="w-fit"
            >
              {updateMe.isPending ? 'Saving…' : 'Save changes'}
            </Button>
          </FieldGroup>
        </form>
      </CardContent>
    </Card>
  )
}
