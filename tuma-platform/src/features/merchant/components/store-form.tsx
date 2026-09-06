/**
 * Store form — "New store" page (coverage §7.13, P18: page, not dialog).
 * Fields: name, description, address, delivery fee with explicit
 * free-delivery handling, category, and the map pin picker — coordinates
 * are stored, never shown (P2/P3). Editing lives on the store detail
 * page (dirty-save there); this form is create-only.
 *
 * Live data: createOwnStore — a real endpoint. New stores start CLOSED
 * (server default); the open-for-orders switch is on the detail page.
 */
import { useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { createOwnStoreMutation } from '@/api/queries'
import {
  Button,
  Card,
  Field,
  Input,
  PageHead,
  Status,
  Textarea,
} from '@/components/ds'
import { PinPicker } from './pin-picker'
import { ApiError } from '@/api/client'

export function StoreNewPage() {
  return <StoreForm />
}

function StoreForm() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [category, setCategory] = useState('')
  const [fee, setFee] = useState('')
  const [pin, setPin] = useState<{ lat: number; lng: number } | null>(null)
  const [errors, setErrors] = useState<Record<string, string>>({})

  const create = useMutation({
    ...createOwnStoreMutation(),
    onSuccess: (store) => {
      void queryClient.invalidateQueries({ queryKey: ['listOwnStores'] })
      toast.success(
        `${store.name} created — it starts closed; open it for orders on its page`,
      )
      void navigate({ to: `/merchant/store/${store.id}` })
    },
    onError: (e) =>
      setErrors({
        form: e instanceof ApiError ? e.message : 'Could not create the store',
      }),
  })

  const submit = () => {
    const next: Record<string, string> = {}
    if (!name.trim()) next.name = 'The store needs its name.'
    if (fee === '' || Number.isNaN(Number(fee)))
      next.fee = 'Set the delivery fee in whole francs — 0 means free.'
    if (!pin) next.pin = 'Drop the pin so riders can find you.'
    setErrors(next)
    if (Object.keys(next).length > 0 || !pin) return

    // The pin names the place: address_text is derived server-side from
    // the pin — the merchant never types an address.
    create.mutate({
      body: {
        name: name.trim(),
        description: description.trim() || null,
        category: category.trim() || null,
        delivery_fee: Number(fee),
        lat: pin.lat,
        lng: pin.lng,
      },
    })
  }

  return (
    <div className="flex flex-col gap-5 pb-10">
      <PageHead
        back={{ to: '/merchant/store', label: 'All stores' }}
        title="New store"
        sub="A store is a physical place customers order from — one branch, one store."
      />

      <Card className="max-w-200">
        <div className="grid gap-4">
          <Field label="Store name" error={errors.name}>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. KFC Kigali Heights"
              invalid={!!errors.name}
            />
          </Field>
          <Field
            label="Description"
            help="One or two lines on the customer's home screen"
          >
            <Textarea
              rows={2}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="What makes this branch worth ordering from."
            />
          </Field>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field
              label="Delivery fee (RWF)"
              help="Whole francs · set 0 for free delivery"
              error={errors.fee}
            >
              <Input
                value={fee}
                inputMode="numeric"
                onChange={(e) => setFee(e.target.value.replace(/\D/g, ''))}
                placeholder="0"
                invalid={!!errors.fee}
              />
            </Field>
            <Field
              label="Category"
              help="Shown on the customer's home screen"
            >
              <Input
                value={category}
                onChange={(e) => setCategory(e.target.value)}
                placeholder="e.g. Fast food"
              />
            </Field>
          </div>
          {fee === '0' ? (
            <div className="text-xs font-medium text-success">
              Free delivery — customers see “Free delivery” on this store.
            </div>
          ) : null}
        </div>
      </Card>

      <Card className="max-w-200">
        <div className="flex items-center gap-3">
          <div className="text-[17px] font-bold">Location pin</div>
          {pin ? (
            <Status tone="success" small>
              Pin dropped
            </Status>
          ) : (
            <Status tone="warning" small>
              No pin yet
            </Status>
          )}
        </div>
        <p className="mt-1 text-[13px] text-text2">
          Drop the pin on the exact pickup spot — riders navigate to it. The
          map position is stored; customers see your address text.
        </p>
        <div className="mt-3">
          <PinPicker pin={pin} onPin={setPin} />
        </div>
        {errors.pin ? (
          <div className="mt-2 text-xs font-medium text-danger" role="alert">
            {errors.pin}
          </div>
        ) : null}
      </Card>

      {errors.form ? (
        <div className="text-xs font-medium text-danger" role="alert">
          {errors.form}
        </div>
      ) : null}

      <div className="flex gap-3">
        <Button variant="primary" onClick={submit} disabled={create.isPending}>
          {create.isPending ? 'Creating…' : 'Create store'}
        </Button>
        <Button
          variant="ghost"
          onClick={() => void navigate({ to: '/merchant/store' })}
        >
          Cancel
        </Button>
      </div>
    </div>
  )
}
