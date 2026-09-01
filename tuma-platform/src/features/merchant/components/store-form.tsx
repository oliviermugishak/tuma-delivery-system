/**
 * Store form — "New store" page and edit page (coverage §7.13/§7.14,
 * P18: pages). Fields: name, description, address, delivery fee with
 * explicit free-delivery handling, category, and the map pin picker —
 * coordinates are stored, never shown (P2/P3).
 *
 * Live data: createOwnStore / updateOwnStore / uploadStoreBanner /
 * deleteStoreBanner — all real endpoints.
 */
import { useEffect, useState } from 'react'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  createOwnStoreMutation,
  getOwnStoreOptions,
  updateOwnStoreMutation,
} from '@/api/queries'
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

const editStoreRouteApi = getRouteApi('/merchant/store/$storeId')

export function StoreNewPage() {
  return <StoreForm mode="create" />
}

export function StoreEditPage() {
  const { storeId } = editStoreRouteApi.useParams()
  return <StoreForm mode="edit" storeId={storeId} />
}

function StoreForm({ mode, storeId }: { mode: 'create' | 'edit'; storeId?: string }) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const existing = useQuery({
    ...getOwnStoreOptions({ path: { id: storeId ?? '' } }),
    enabled: mode === 'edit' && !!storeId,
  })

  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [address, setAddress] = useState('')
  const [category, setCategory] = useState('')
  const [fee, setFee] = useState('')
  const [pin, setPin] = useState<{ lat: number; lng: number } | null>(null)
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [seeded, setSeeded] = useState(false)

  useEffect(() => {
    if (mode === 'edit' && existing.data && !seeded) {
      setName(existing.data.name)
      setDescription(existing.data.description ?? '')
      setAddress(existing.data.address_text ?? '')
      setCategory(existing.data.category ?? '')
      setFee(String(existing.data.delivery_fee))
      setPin(
        existing.data.lat != null && existing.data.lng != null
          ? { lat: existing.data.lat, lng: existing.data.lng }
          : null,
      )
      setSeeded(true)
    }
  }, [mode, existing.data, seeded])

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ['listOwnStores'] })
    void queryClient.invalidateQueries({ queryKey: ['getOwnStore'] })
  }

  const create = useMutation({
    ...createOwnStoreMutation(),
    onSuccess: (store) => {
      invalidate()
      toast.success(`${store.name} created — add products so customers can order`)
      void navigate({ to: `/merchant/store/${store.id}` })
    },
    onError: (e) =>
      setErrors({ form: e instanceof ApiError ? e.message : 'Could not create the store' }),
  })

  const update = useMutation({
    ...updateOwnStoreMutation(),
    onSuccess: (store) => {
      invalidate()
      toast.success('Store saved')
      void navigate({ to: `/merchant/store/${store.id}` })
    },
    onError: (e) =>
      setErrors({ form: e instanceof ApiError ? e.message : 'Could not save the store' }),
  })

  const submit = () => {
    const next: Record<string, string> = {}
    if (!name.trim()) next.name = 'The store needs its name.'
    if (!address.trim()) next.address = 'Customers need an address to find you.'
    if (fee === '' || Number.isNaN(Number(fee)))
      next.fee = 'Set the delivery fee in whole francs — 0 means free.'
    if (mode === 'create' && !pin)
      next.pin = 'Drop the pin so riders can find you.'
    setErrors(next)
    if (Object.keys(next).length > 0) return

    const body = {
      name: name.trim(),
      description: description.trim() || null,
      address_text: address.trim(),
      category: category.trim() || null,
      delivery_fee: Number(fee),
      ...(pin ? { lat: pin.lat, lng: pin.lng } : {}),
    }

    if (mode === 'create' && pin) {
      create.mutate({ body: { ...body, lat: pin.lat, lng: pin.lng } })
    } else if (mode === 'edit' && storeId) {
      update.mutate({ path: { id: storeId }, body })
    }
  }

  const pending = create.isPending || update.isPending

  return (
    <div className="flex flex-col gap-5 pb-10">
      <PageHead
        back={{ to: '/merchant/store', label: 'All stores' }}
        title={mode === 'create' ? 'New store' : 'Edit store'}
        sub={
          mode === 'create'
            ? 'A store is a physical place customers order from — one branch, one store.'
            : undefined
        }
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
          <Field label="Description" help="One or two lines on the customer's home screen">
            <Textarea
              rows={2}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="What makes this branch worth ordering from."
            />
          </Field>
          <Field label="Address" error={errors.address}>
            <Input
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              placeholder="Street, sector, landmark people know"
              invalid={!!errors.address}
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
              error={errors.category}
            >
              <Input
                value={category}
                onChange={(e) => setCategory(e.target.value)}
                placeholder="e.g. Fast food"
              />
            </Field>
          </div>
          {fee === '0' && mode === 'create' ? (
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
        <Button variant="primary" onClick={submit} disabled={pending}>
          {pending ? 'Saving…' : mode === 'create' ? 'Create store' : 'Save changes'}
        </Button>
        <Button variant="ghost" onClick={() => void navigate({ to: '/merchant/store' })}>
          Cancel
        </Button>
      </div>
    </div>
  )
}
