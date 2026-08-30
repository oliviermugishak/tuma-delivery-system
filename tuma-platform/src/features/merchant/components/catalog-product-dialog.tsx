import { useEffect, useRef, useState } from 'react'
import { ImagePlus, X } from 'lucide-react'

import type { ProductResponse } from '@/api/generated'
import { FileDrop } from '@/components/file-drop'
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
import { toast } from 'sonner'
import { ProductGalleryEditor } from '@/features/merchant/components/product-gallery-editor'
import {
  useCreateCatalogProduct,
} from '@/features/merchant/hooks/use-create-product'
import {
  useUpdateCatalogProduct,
} from '@/features/merchant/hooks/use-update-product'
import { useUploadProductImage } from '@/features/merchant/hooks/use-upload-product-image'

const ACCEPTED = 'image/jpeg,image/png,image/webp'

/** A file picked in create mode, held locally until the product exists. */
interface PendingImage {
  key: string
  file: File
  previewUrl: string
}

/**
 * Add/edit CATALOG product dialog — one form for both. The catalog holds
 * the product's identity (name, description); prices live per store and
 * are set when the product is attached to a store's assortment. On
 * failure the dialog stays open with the values intact.
 *
 * Imagery lives in the GALLERY only — there is deliberately no URL input,
 * so an uploaded cover can never be clobbered by a pasted link. Create
 * mode collects files locally and uploads them right after the product
 * exists (sequential, with per-image status); edit mode manages the
 * server-side gallery directly.
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
  const uploadImage = useUploadProductImage()
  const pending = createProduct.isPending || updateProduct.isPending

  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [pendingImages, setPendingImages] = useState<PendingImage[]>([])
  const [uploading, setUploading] = useState(false)
  const fileInput = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (open) {
      setName(product?.name ?? '')
      setDescription(product?.description ?? '')
      setPendingImages([])
    }
  }, [open, product])

  // Release the object URLs when the dialog closes or a preview goes.
  useEffect(() => {
    if (!open) {
      for (const image of pendingImages) URL.revokeObjectURL(image.previewUrl)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  const addFiles = (files: File[]) => {
    const next = files.slice(0, 8 - pendingImages.length)
    if (next.length === 0) {
      toast.error('A gallery holds at most 8 images')
      return
    }
    setPendingImages((current) => [
      ...current,
      ...next.map((file) => ({
        key: `${file.name}-${file.lastModified}-${Math.random()}`,
        file,
        previewUrl: URL.createObjectURL(file),
      })),
    ])
  }

  const removePending = (key: string) => {
    setPendingImages((current) => {
      const target = current.find((image) => image.key === key)
      if (target) URL.revokeObjectURL(target.previewUrl)
      return current.filter((image) => image.key !== key)
    })
  }

  const closeAndReset = () => {
    for (const image of pendingImages) URL.revokeObjectURL(image.previewUrl)
    setPendingImages([])
    onOpenChange(false)
  }

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    // No image_url in the body, ever — the gallery is the only imagery
    // surface, so a pasted link can never hang off a product.
    const body = {
      name: name.trim(),
      description: description.trim() || null,
    }
    if (product) {
      updateProduct.mutate(
        { path: { id: product.id }, body },
        { onSuccess: () => closeAndReset() },
      )
      return
    }

    createProduct.mutate(
      { body },
      {
        onSuccess: async (created) => {
          if (pendingImages.length === 0) {
            closeAndReset()
            return
          }
          // Create → upload each picked file sequentially. A mid-queue
          // failure never blocks the rest; the merchant retries in Edit.
          setUploading(true)
          let failed = 0
          for (const image of pendingImages) {
            try {
              await uploadImage.mutateAsync({
                path: { id: created.id },
                body: { file: image.file },
              })
            } catch {
              failed += 1
            }
          }
          setUploading(false)
          toast.success(
            failed === 0
              ? `Product created with ${pendingImages.length} image${pendingImages.length > 1 ? 's' : ''}`
              : `Product created — ${failed} image${failed > 1 ? 's' : ''} failed; retry from Edit`,
          )
          closeAndReset()
        },
      },
    )
  }

  const busy = pending || uploading

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
            {product ? (
              <Field>
                <FieldLabel>Gallery</FieldLabel>
                <ProductGalleryEditor productId={product.id} />
              </Field>
            ) : (
              <Field>
                <FieldLabel>Images</FieldLabel>
                <FileDrop onFiles={addFiles} disabled={busy}>
                  <input
                    ref={fileInput}
                    type="file"
                    accept={ACCEPTED}
                    multiple
                    className="hidden"
                    onChange={(e) => {
                      addFiles(Array.from(e.target.files ?? []))
                      e.target.value = ''
                    }}
                  />
                  <div className="rounded-xl border border-dashed p-4 text-center">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      disabled={busy}
                      onClick={() => fileInput.current?.click()}
                    >
                      <ImagePlus data-icon="inline-start" />
                      Choose images
                    </Button>
                    <p className="mt-2 text-xs text-muted-foreground">
                      Or drop them here — JPEG, PNG or WebP, up to 8 per
                      product; the server resizes for you.
                    </p>
                  </div>
                </FileDrop>
                {pendingImages.length > 0 && (
                  <div className="mt-3 grid grid-cols-4 gap-2">
                    {pendingImages.map((image) => (
                      <div
                        key={image.key}
                        className="group relative aspect-square overflow-hidden rounded-lg border bg-muted/30"
                      >
                        <img
                          src={image.previewUrl}
                          alt={image.file.name}
                          className="h-full w-full object-cover"
                        />
                        <button
                          type="button"
                          aria-label={`Remove ${image.file.name}`}
                          className="absolute top-1 right-1 flex size-5 items-center justify-center rounded-full bg-background/85 text-foreground hover:text-destructive"
                          onClick={() => removePending(image.key)}
                        >
                          <X className="size-3" />
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </Field>
            )}
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => closeAndReset()}
                disabled={busy}
              >
                Cancel
              </Button>
              <Button type="submit" disabled={busy || !name.trim()}>
                {uploading
                  ? 'Uploading images…'
                  : pending
                    ? 'Saving…'
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
