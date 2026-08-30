import { useRef, useState } from 'react'
import { ImagePlus, Star, Trash2 } from 'lucide-react'

import type { ProductImageResponse } from '@/api/generated'
import { FileDrop } from '@/components/file-drop'
import { ImageWithFallback } from '@/components/image-with-fallback'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useDeleteProductImage } from '@/features/merchant/hooks/use-delete-product-image'
import { useProductImages } from '@/features/merchant/hooks/use-product-images'
import { useSetProductCover } from '@/features/merchant/hooks/use-set-product-cover'
import { useUploadProductImage } from '@/features/merchant/hooks/use-upload-product-image'

const ACCEPTED = 'image/jpeg,image/png,image/webp'

/**
 * The gallery editor inside the catalog edit dialog: thumbnails in
 * gallery order (cover first), an upload button, and per-image
 * make-cover / remove. Uploads post immediately — the gallery lives on
 * the server, not in this form's state. Create-mode dialogs don't render
 * this (a product must exist before images can attach to it).
 */
export function ProductGalleryEditor({ productId }: { productId: string }) {
  const gallery = useProductImages(productId)
  const upload = useUploadProductImage()
  const setCover = useSetProductCover()
  const remove = useDeleteProductImage()
  const [deleting, setDeleting] = useState<ProductImageResponse | null>(null)
  const fileInput = useRef<HTMLInputElement>(null)

  const onPickFile = (files: FileList | null) => {
    const file = files?.[0]
    if (!file) return
    upload.mutate(
      { path: { id: productId }, body: { file } },
      { onSettled: () => {
        if (fileInput.current) fileInput.current.value = ''
      } },
    )
  }

  return (
    <FileDrop
      className="space-y-3"
      disabled={upload.isPending}
      onFiles={(files) => {
        for (const file of files) {
          upload.mutate({ path: { id: productId }, body: { file } })
        }
      }}
    >
      <input
        ref={fileInput}
        type="file"
        accept={ACCEPTED}
        className="hidden"
        onChange={(e) => onPickFile(e.target.files)}
      />
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-muted-foreground">
          The first image is the cover customers see. JPEG, PNG or WebP; up
          to 5&nbsp;MB, 8 per product — the server resizes for you.
        </p>
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={upload.isPending}
          onClick={() => fileInput.current?.click()}
        >
          <ImagePlus data-icon="inline-start" />
          {upload.isPending ? 'Uploading…' : 'Add image'}
        </Button>
      </div>

      {gallery.isPending ? (
        <div className="grid grid-cols-3 gap-3">
          <Skeleton className="aspect-[4/3] rounded-lg" />
          <Skeleton className="aspect-[4/3] rounded-lg" />
          <Skeleton className="aspect-[4/3] rounded-lg" />
        </div>
      ) : gallery.data && gallery.data.length > 0 ? (
        <div className="grid grid-cols-3 gap-3">
          {gallery.data.map((image) => (
            <div
              key={image.id}
              className="group relative overflow-hidden rounded-lg border bg-muted/30"
            >
              <div className="aspect-[4/3] w-full">
                <ImageWithFallback src={image.image_url} alt="Product image" />
              </div>
              {image.position === 0 && (
                <span className="absolute top-1.5 left-1.5 flex items-center gap-1 rounded-md bg-primary px-1.5 py-0.5 text-xs font-semibold text-primary-foreground">
                  <Star className="size-3" aria-hidden />
                  Cover
                </span>
              )}
              <div className="absolute inset-x-0 bottom-0 flex justify-end gap-1 bg-background/80 p-1.5 opacity-0 transition-opacity group-hover:opacity-100">
                {image.position !== 0 && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-7 px-2 text-xs"
                    disabled={setCover.isPending}
                    onClick={() =>
                      setCover.mutate({
                        path: { product_id: productId, image_id: image.id },
                      })
                    }
                  >
                    <Star className="size-3.5" />
                    Make cover
                  </Button>
                )}
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-muted-foreground hover:text-destructive"
                  disabled={remove.isPending}
                  aria-label="Remove image"
                  onClick={() => setDeleting(image)}
                >
                  <Trash2 className="size-3.5" />
                </Button>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground">
          No images yet — drop photos anywhere here, or use the button.
          One good photo of the food sells more than any description.
        </div>
      )}

      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open && !remove.isPending) setDeleting(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Remove this image?</AlertDialogTitle>
            <AlertDialogDescription>
              It disappears from the gallery and from everywhere the product
              is shown. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Keep it</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => {
                if (deleting === null) return
                remove.mutate(
                  {
                    path: {
                      product_id: productId,
                      image_id: deleting.id,
                    },
                  },
                  { onSettled: () => setDeleting(null) },
                )
              }}
            >
              Remove
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </FileDrop>
  )
}
