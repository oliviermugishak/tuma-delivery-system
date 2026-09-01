/**
 * Admin — "Add rider" page (coverage §7.5, P18: page, not dialog).
 * POST /v1/admin/riders provisions the OTP account (phone) + rider
 * profile with its server-generated rider number in one transaction.
 */
import { useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { createRiderMutation } from '@/api/queries'
import { Button, Card, Field, Input, PageHead } from '@/components/ds'
import { ApiError } from '@/api/client'

export function RiderNewPage() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [name, setName] = useState('')
  const [phone, setPhone] = useState('')
  const [errors, setErrors] = useState<Record<string, string>>({})

  const create = useMutation({
    ...createRiderMutation(),
    onSuccess: (rider) => {
      void queryClient.invalidateQueries({ queryKey: ['listRiders'] })
      toast.success(
        `${rider.name} added as Rider #${rider.rider_number} — they sign in with their phone`,
      )
      void navigate({ to: '/admin/riders' })
    },
    onError: (e) => {
      if (e instanceof ApiError && e.status === 409) {
        setErrors({ phone: 'That phone number already has a rider' })
      } else {
        setErrors({
          form: e instanceof ApiError ? e.message : 'Could not add the rider',
        })
      }
    },
  })

  const submit = () => {
    const next: Record<string, string> = {}
    if (!name.trim()) next.name = 'The rider needs a name.'
    const digits = phone.replace(/\D/g, '')
    if (digits.length < 9) next.phone = 'Enter the full phone number.'
    setErrors(next)
    if (Object.keys(next).length > 0) return
    create.mutate({ body: { name: name.trim(), phone: phone.trim() } })
  }

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        back={{ to: '/admin/riders', label: 'All riders' }}
        title="Add rider"
        sub="The rider gets a unique number — merchants type it at hand-off. They sign in with their phone and an OTP code."
      />

      <Card className="max-w-150">
        <div className="grid gap-4">
          <Field label="Full name" error={errors.name}>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Billie Jean"
              invalid={!!errors.name}
            />
          </Field>
          <Field
            label="Phone"
            help="Their sign-in — the OTP goes here"
            error={errors.phone}
          >
            <Input
              type="tel"
              value={phone}
              onChange={(e) => setPhone(e.target.value)}
              placeholder="+250 788 000 000"
              invalid={!!errors.phone}
            />
          </Field>
          {errors.form ? (
            <div className="text-xs font-medium text-danger" role="alert">
              {errors.form}
            </div>
          ) : null}
          <div className="mt-2 flex gap-3">
            <Button
              variant="primary"
              onClick={submit}
              disabled={create.isPending}
            >
              {create.isPending ? 'Adding…' : 'Add rider'}
            </Button>
            <Button variant="ghost" onClick={() => window.history.back()}>
              Cancel
            </Button>
          </div>
        </div>
      </Card>
    </div>
  )
}
