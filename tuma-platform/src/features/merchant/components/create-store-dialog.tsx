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
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { useCreateStore } from '@/features/merchant/hooks/use-create-store'
import { StoreLocationFields } from '@/features/merchant/components/store-location-fields'

/**
 * Create one of the merchant's stores — merchants can have several, so this
 * dialog stays reachable from the stores list at any time. New stores start
 * closed; opening is a deliberate act on the store's detail page. Delivery
 * fee is integer RWF — left empty it means free delivery (server default).
 * Coordinates + category are the tracking contract: real coordinates power
 * real distances and ETAs. On failure the dialog stays open with the values
 * intact.
 */
export function CreateStoreDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const createStore = useCreateStore()

  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [address, setAddress] = useState('')
  const [fee, setFee] = useState('')
  const [category, setCategory] = useState('')
  const [lat, setLat] = useState('')
  const [lng, setLng] = useState('')

  useEffect(() => {
    if (open) {
      setName('')
      setDescription('')
      setAddress('')
      setFee('')
      setCategory('')
      setLat('')
      setLng('')
    }
  }, [open])

  const feeNumber = fee.trim() === '' ? null : Number.parseInt(fee, 10)
  const feeValid =
    feeNumber === null || (Number.isFinite(feeNumber) && feeNumber >= 0)

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!feeValid || !name.trim()) return
    createStore.mutate(
      {
        body: {
          name: name.trim(),
          description: description.trim(),
          address_text: address.trim(),
          ...(feeNumber !== null ? { delivery_fee: feeNumber } : {}),
          ...(category.trim() ? { category: category.trim() } : {}),
          ...(lat.trim() !== '' ? { lat: Number(lat) } : {}),
          ...(lng.trim() !== '' ? { lng: Number(lng) } : {}),
        },
      },
      { onSuccess: () => onOpenChange(false) },
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Create a store</DialogTitle>
          <DialogDescription>
            A store is what customers see — its name, where it is, and what
            delivery costs. You can change everything later.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="store-name">Store name</FieldLabel>
              <Input
                id="store-name"
                autoComplete="off"
                required
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Aline's Kitchen"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="store-description">Description</FieldLabel>
              <Textarea
                id="store-description"
                maxLength={1000}
                rows={3}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="What you cook, and what makes it good."
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="store-address">Address</FieldLabel>
              <Input
                id="store-address"
                autoComplete="off"
                maxLength={200}
                value={address}
                onChange={(e) => setAddress(e.target.value)}
                placeholder="KN 4 Ave, Kigali"
              />
              <FieldDescription>
                Where customers' orders come from.
              </FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="store-fee">Delivery fee (RWF)</FieldLabel>
              <Input
                id="store-fee"
                type="number"
                min={0}
                step={1}
                inputMode="numeric"
                value={fee}
                onChange={(e) => setFee(e.target.value)}
                placeholder="1500"
              />
              <FieldDescription>
                Whole francs. Leave empty for free delivery.
              </FieldDescription>
            </Field>
            <StoreLocationFields
              category={category}
              onCategoryChange={setCategory}
              lat={lat}
              onLatChange={setLat}
              lng={lng}
              onLngChange={setLng}
            />
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={createStore.isPending}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={createStore.isPending || !feeValid || !name.trim()}
              >
                {createStore.isPending ? 'Creating…' : 'Create store'}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}
