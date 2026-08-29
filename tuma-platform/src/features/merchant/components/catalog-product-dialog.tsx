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
import { Field, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import {
  useCreateCatalogProduct,
} from '@/features/merchant/hooks/use-create-product'
import {
  useUpdateCatalogProduct,
} from '@/features/merchant/hooks/use-update-product'

/**
 * Add/edit CATALOG product dialog — one form for both. The catalog holds
 * the product's identity (name, description, image); prices live per
 * store and are set when the product is attached to a store's assortment.
 * On failure the dialog stays open with the values intact.
 */
export function CatalogProductDialog({
  open,
  onOpenChange,
  product,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  product?: ProductResponse | null
}) {
  const createProduct = useCreateCatalogProduct()
  const updateProduct = useUpdateCatalogProduct()
  const pending = createProduct.isPending || updateProduct.isPending

  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [imageUrl, setImageUrl] = useState('')

  useEffect(() => {
    if (open) {
      setName(product?.name ?? '')
      setDescription(product?.description ?? '')
      setImageUrl(product?.image_url ?? '')
    }
  }, [open, product])

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    const body = {
      name: name.trim(),
      description: description.trim() || null,
      image_url: imageUrl.trim() || null,
    }
    if (product) {
      updateProduct.mutate(
        { path: { id: product.id }, body },
        { onSuccess: () => onOpenChange(false) },
      )
    } else {
      createProduct.mutate(
        { body },
        { onSuccess: () => onOpenChange(false) },
      )
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {product ? 'Edit catalog product' : 'New catalog product'}
          </DialogTitle>
          <DialogDescription>
            The product&apos;s identity, defined once. Each store sets its
            own price and stock when the product joins its assortment.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="catalog-name">Name</FieldLabel>
              <Input
                id="catalog-name"
                autoComplete="off"
                required
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Rice 5KG"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="catalog-description">
                Description
              </FieldLabel>
              <Textarea
                id="catalog-description"
                maxLength={1000}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="What is it? Why is it good?"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="catalog-image">Image URL</FieldLabel>
              <Input
                id="catalog-image"
                autoComplete="off"
                value={imageUrl}
                onChange={(e) => setImageUrl(e.target.value)}
                placeholder="https://…"
              />
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
              <Button type="submit" disabled={pending || !name.trim()}>
                {pending ? 'Saving…' : product ? 'Save changes' : 'Add product'}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}
