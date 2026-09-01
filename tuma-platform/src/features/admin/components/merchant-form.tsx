/**
 * Admin — "Add merchant" page (coverage §7.3, P18: page, not dialog) and
 * the shared edit page. Create provisions the business + owner account +
 * owner membership in one server transaction (POST /v1/admin/merchants).
 * The owner signs into the merchant wing with that account.
 */
import { useEffect, useState } from 'react'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  createMerchantMutation,
  getMerchantOptions,
  updateMerchantMutation,
} from '@/api/queries'
import { Button, Card, Field, Input, PageHead } from '@/components/ds'
import { ApiError } from '@/api/client'

const editRouteApi = getRouteApi('/admin/merchants/$merchantId/edit')

export function MerchantNewPage() {
  return <MerchantForm mode="create" />
}

export function MerchantEditPage() {
  const { merchantId } = editRouteApi.useParams()
  return <MerchantForm mode="edit" merchantId={merchantId} />
}

function MerchantForm({
  mode,
  merchantId,
}: {
  mode: 'create' | 'edit'
  merchantId?: string
}) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const existing = useQuery({
    ...getMerchantOptions({ path: { id: merchantId ?? '' } }),
    enabled: mode === 'edit' && !!merchantId,
  })

  const [name, setName] = useState('')
  const [businessEmail, setBusinessEmail] = useState('')
  const [businessPhone, setBusinessPhone] = useState('')
  const [ownerEmail, setOwnerEmail] = useState('')
  const [ownerPassword, setOwnerPassword] = useState('')
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [seeded, setSeeded] = useState(false)

  useEffect(() => {
    if (mode === 'edit' && existing.data && !seeded) {
      setName(existing.data.name)
      setBusinessEmail(existing.data.business_email ?? '')
      setBusinessPhone(existing.data.business_phone ?? '')
      setSeeded(true)
    }
  }, [mode, existing.data, seeded])

  const create = useMutation({
    ...createMerchantMutation(),
    onSuccess: (created) => {
      void queryClient.invalidateQueries({ queryKey: ['listMerchants'] })
      toast.success(`${created.name} added — the owner signs in with their email`)
      void navigate({ to: `/admin/merchants/${created.id}` })
    },
    onError: (e) => {
      if (e instanceof ApiError && e.status === 409) {
        setErrors({ ownerEmail: 'That email already has an account' })
      } else {
        setErrors({ form: e instanceof ApiError ? e.message : 'Could not add the merchant' })
      }
    },
  })

  const update = useMutation({
    ...updateMerchantMutation(),
    onSuccess: (updated) => {
      void queryClient.invalidateQueries({ queryKey: ['listMerchants'] })
      void queryClient.invalidateQueries({ queryKey: ['getMerchant'] })
      toast.success('Merchant updated')
      void navigate({ to: `/admin/merchants/${updated.id}` })
    },
    onError: (e) =>
      setErrors({ form: e instanceof ApiError ? e.message : 'Could not update the merchant' }),
  })

  const submit = () => {
    const next: Record<string, string> = {}
    if (!name.trim()) next.name = 'Give the business its name.'
    if (mode === 'create') {
      if (!ownerEmail.trim()) next.ownerEmail = 'The owner needs a sign-in email.'
      if (ownerPassword.length < 8)
        next.ownerPassword = 'At least 8 characters.'
    }
    setErrors(next)
    if (Object.keys(next).length > 0) return

    if (mode === 'create') {
      create.mutate({
        body: {
          name: name.trim(),
          business_email: businessEmail.trim() || null,
          email: ownerEmail.trim(),
          password: ownerPassword,
        },
      })
    } else if (merchantId) {
      update.mutate({
        path: { id: merchantId },
        body: {
          name: name.trim(),
          business_email: businessEmail.trim() || null,
          business_phone: businessPhone.trim() || null,
          // Status is deliberately untouched here — suspension happens
          // behind its own guard on the list/detail surfaces.
          status: existing.data?.status ?? 'active',
        },
      })
    }
  }

  const pending = create.isPending || update.isPending

  return (
    <div className="flex flex-col gap-5 pb-20">
      <PageHead
        back={{
          to: merchantId ? `/admin/merchants/${merchantId}` : '/admin/merchants',
          label: merchantId ? 'Merchant' : 'All merchants',
        }}
        title={mode === 'create' ? 'Add merchant' : 'Edit merchant'}
        sub={
          mode === 'create'
            ? 'Creates the business and its owner account. The owner signs into the merchant wing with that email.'
            : undefined
        }
      />

      <Card className="max-w-200">
        <div className="grid gap-4">
          <Field label="Business name" error={errors.name}>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. KFC Rwanda"
              invalid={!!errors.name}
              onBlur={() =>
                !name.trim() &&
                setErrors((prev) => ({ ...prev, name: 'Give the business its name.' }))
              }
            />
          </Field>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field
              label="Business email"
              help="Optional — the public contact on the store"
              error={errors.businessEmail}
            >
              <Input
                type="email"
                value={businessEmail}
                onChange={(e) => setBusinessEmail(e.target.value)}
                placeholder="info@business.rw"
              />
            </Field>
            <Field label="Business phone" help="Optional">
              <Input
                type="tel"
                value={businessPhone}
                onChange={(e) => setBusinessPhone(e.target.value)}
                placeholder="+250 788 000 000"
              />
            </Field>
          </div>

          {mode === 'create' ? (
            <>
              <div className="mt-2 border-t border-white/8 pt-4 text-[15px] font-semibold">
                Owner account
              </div>
              <div className="grid gap-4 sm:grid-cols-2">
                <Field
                  label="Owner email"
                  help="Their sign-in for the merchant wing"
                  error={errors.ownerEmail}
                >
                  <Input
                    type="email"
                    value={ownerEmail}
                    onChange={(e) => setOwnerEmail(e.target.value)}
                    placeholder="owner@business.rw"
                    invalid={!!errors.ownerEmail}
                  />
                </Field>
                <Field
                  label="Temporary password"
                  help="At least 8 characters — they change it after sign-in"
                  error={errors.ownerPassword}
                >
                  <Input
                    type="text"
                    value={ownerPassword}
                    onChange={(e) => setOwnerPassword(e.target.value)}
                    placeholder="Set a starting password"
                    invalid={!!errors.ownerPassword}
                  />
                </Field>
              </div>
            </>
          ) : null}

          {errors.form ? (
            <div className="text-xs font-medium text-danger" role="alert">
              {errors.form}
            </div>
          ) : null}

          <div className="mt-2 flex gap-3">
            <Button variant="primary" onClick={submit} disabled={pending}>
              {pending
                ? 'Saving…'
                : mode === 'create'
                  ? 'Add merchant'
                  : 'Save changes'}
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
