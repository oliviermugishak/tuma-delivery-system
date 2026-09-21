/**
 * The product gallery editor — upload (FileDrop or file picker), set the
 * cover, remove (guarded). Shared by Catalog edit and the product form;
 * one gallery vocabulary.
 *
 * Live data: /v1/merchant/products/{id}/images — upload, delete, cover.
 * The parent owns the listProductImages query and passes the images down;
 * every mutation invalidates that key so the parent refetches.
 */
import { useRef, useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  deleteProductImageMutation,
  setProductCoverMutation,
  uploadProductImageMutation,
} from '@/api/queries'
import { Button, GuardDialog, Icon } from '@/components/ds'
import { FileDrop } from '@/components/file-drop'
import { ApiError } from '@/api/client'

export interface GalleryImage {
  id: string
  image_url: string
  position: number
}

export function GalleryEditor({
  productId,
  images,
}: {
  productId: string
  images: GalleryImage[]
}) {
  const queryClient = useQueryClient()
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const invalidate = () =>
    void queryClient.invalidateQueries({ queryKey: ['listProductImages'] })

  const upload = useMutation({
    ...uploadProductImageMutation(),
    onSuccess: () => {
      invalidate()
      toast.success('Image uploaded')
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not upload the image'),
  })

  const remove = useMutation({
    ...deleteProductImageMutation(),
    onSuccess: () => {
      invalidate()
      toast.success('Image removed')
      setDeleteTarget(null)
    },
    onError: (e) => {
      setDeleteTarget(null)
      toast.error(e instanceof ApiError ? e.message : 'Could not remove the image')
    },
  })

  const setCover = useMutation({
    ...setProductCoverMutation(),
    onSuccess: () => {
      invalidate()
      toast.success('Cover updated')
    },
    onError: () => toast.error('Could not set the cover'),
  })

  const uploadFiles = (files: File[]) => {
    // Every dropped/selected image uploads — one mutation each so a
    // single failure doesn't take the batch down.
    for (const file of files) {
      upload.mutate({ path: { id: productId }, body: { file } })
    }
  }

  const openPicker = () => fileInputRef.current?.click()

  // The cover is the server's position 0, not this array's index.
  const coverId = images.find((img) => img.position === 0)?.id

  return (
    <div className="mt-3">
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        className="hidden"
        onChange={(e) => {
          const files = Array.from(e.target.files ?? [])
          if (files.length > 0) uploadFiles(files)
          e.target.value = ''
        }}
      />
      <FileDrop onFiles={uploadFiles} disabled={upload.isPending}>
        <button
          type="button"
          onClick={openPicker}
          disabled={upload.isPending}
          className="flex h-24 w-full flex-col items-center justify-center gap-1.5 rounded-xl border border-dashed border-line text-center text-[12.5px] text-text3 hover:bg-high"
        >
          <Icon name="upload" label="" size={20} />
          {upload.isPending ? 'Uploading…' : 'Click or drop images'}
        </button>
      </FileDrop>
      <div className="mt-2 flex justify-end">
        <Button
          small
          variant="outline"
          disabled={upload.isPending}
          onClick={openPicker}
        >
          Upload
        </Button>
      </div>

      {images.length === 0 ? (
        <div className="mt-2 text-[13px] text-text2">No images yet.</div>
      ) : (
        <div className="mt-3 grid grid-cols-3 gap-2">
          {images.map((img) => (
            <div
              key={img.id}
              className="overflow-hidden rounded-lg border border-line bg-surface"
            >
              <img
                src={img.image_url}
                alt=""
                className="aspect-square w-full object-cover"
              />
              <div className="flex items-center justify-between gap-1 px-2 py-1.5">
                {img.id === coverId ? (
                  <span className="text-[11px] font-semibold text-brand">
                    Cover
                  </span>
                ) : (
                  <button
                    type="button"
                    className="text-[11px] font-semibold text-text2 hover:text-foreground"
                    onClick={() =>
                      setCover.mutate({
                        path: { product_id: productId, image_id: img.id },
                      })
                    }
                  >
                    Set cover
                  </button>
                )}
                <button
                  type="button"
                  className="text-[11px] font-semibold text-danger hover:text-danger"
                  onClick={() => setDeleteTarget(img.id)}
                >
                  Remove
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <GuardDialog
        open={deleteTarget != null}
        onClose={() => setDeleteTarget(null)}
        title="Remove this image?"
        confirmLabel="Remove image"
        pending={remove.isPending}
        onConfirm={() => {
          if (deleteTarget)
            remove.mutate({
              path: { product_id: productId, image_id: deleteTarget },
            })
        }}
        note="The image leaves every store's menu immediately."
      >
        This removes the image from the product's gallery.
      </GuardDialog>
    </div>
  )
}
