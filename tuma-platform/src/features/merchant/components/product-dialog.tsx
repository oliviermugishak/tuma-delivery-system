import { useEffect, useState } from 'react'

import type { ProductResponse } from '@/api/generated'
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import { useCreateProduct } from '@/features/merchant/hooks/use-create-product'
import { useOwnStores } from '@/features/merchant/hooks/use-stores'
import { useUpdateProduct } from '@/features/merchant/hooks/use-update-product'

/**
 * Add/edit product dialog — one form for both. Without `product` it
 * creates (the merchant picks which of their stores the product joins,
 * preselected to the first store); with one it edits, and the store is a
 * read-only fact — products don't move between stores in V1. Prices are
 * integer RWF. On failure the dialog stays open with the values intact.
 */
export function ProductDialog({
  open,
  onOpenChange,
  product,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  product?: ProductResponse | null
}) {
  const stores = useOwnStores()
  const createProduct = useCreateProduct()
  const updateProduct = useUpdateProduct()
  const pending = createProduct.isPending || updateProduct.isPending

  const [storeId, setStoreId] = useState('')
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [price, setPrice] = useState('')
  const [available, setAvailable] = useState(true)

  useEffect(() => {
    if (open) {
      setStoreId(product?.store_id ?? (stores.data?.[0]?.id ?? ''))
      setName(product?.name ?? '')
      setDescription(product?.description ?? '')
      setPrice(product ? String(product.price) : '')
      setAvailable(product?.is_available ?? true)
    }
  }, [open, product, stores.data])

  const priceNumber = Number.parseInt(price, 10)
  const priceValid =
    price.trim() !== '' && Number.isFinite(priceNumber) && priceNumber >= 0

  const productStore = stores.data?.find((s) => s.id === product?.store_id)

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!priceValid || !name.trim()) return
    if (product) {
      updateProduct.mutate(
        {
          path: { id: product.id },
          body: {
            name: name.trim(),
            description: description.trim(),
            price: priceNumber,
            is_available: available,
          },
        },
        { onSuccess: () => onOpenChange(false) },
      )
    } else {
      if (!storeId) return
      createProduct.mutate(
        {
          body: {
            store_id: storeId,
            name: name.trim(),
            description: description.trim(),
            price: priceNumber,
            is_available: available,
          },
        },
        { onSuccess: () => onOpenChange(false) },
      )
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{product ? 'Edit product' : 'Add product'}</DialogTitle>
          <DialogDescription>
            {product
              ? 'Update this item on your menu.'
              : 'Add an item to your menu. Customers see it while the store is open.'}
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            <Field>
              <FieldLabel>Store</FieldLabel>
              {product ? (
                <p className="rounded-lg border bg-muted/20 px-3 py-2 text-sm font-medium">
                  {productStore?.name ?? '—'}
                </p>
              ) : (
                <Select
                  value={storeId}
                  onValueChange={(value) => {
                    if (value !== null) setStoreId(value)
                  }}
                >
                  <SelectTrigger id="product-store" className="w-full">
                    {/* Base UI resolves item labels only while the popup is
                        mounted, so the closed trigger would show the raw id —
                        feed it the store name explicitly. */}
                    <SelectValue placeholder="Choose a store">
                      {stores.data?.find((s) => s.id === storeId)?.name}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {(stores.data ?? []).map((store) => (
                      <SelectItem key={store.id} value={store.id}>
                        {store.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
              <FieldDescription>
                {product
                  ? 'Products stay with the store they were added to.'
                  : 'Which of your stores this product belongs to.'}
              </FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="product-name">Name</FieldLabel>
              <Input
                id="product-name"
                autoComplete="off"
                required
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Ibirazi"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="product-description">Description</FieldLabel>
              <Textarea
                id="product-description"
                maxLength={1000}
                rows={3}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Rice and beans, fresh and hot."
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="product-price">Price (RWF)</FieldLabel>
              <Input
                id="product-price"
                type="number"
                min={0}
                step={1}
                inputMode="numeric"
                required
                value={price}
                onChange={(e) => setPrice(e.target.value)}
                placeholder="3500"
              />
              <FieldDescription>Whole francs, 0 or more.</FieldDescription>
            </Field>
            <Field>
              <div className="flex items-center justify-between gap-4 rounded-lg border p-4">
                <div className="grid gap-0.5">
                  <FieldLabel htmlFor="product-available">Available</FieldLabel>
                  <p className="text-xs text-muted-foreground">
                    Customers can order this item while it&apos;s on.
                  </p>
                </div>
                <Switch
                  id="product-available"
                  checked={available}
                  onCheckedChange={setAvailable}
                  disabled={pending}
                />
              </div>
            </Field>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={pending}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={
                  pending || !priceValid || !name.trim() || (!product && !storeId)
                }
              >
                {pending
                  ? product
                    ? 'Saving…'
                    : 'Adding…'
                  : product
                    ? 'Save changes'
                    : 'Add product'}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}
