import { useRef } from 'react'
import { ImagePlus, Trash2 } from 'lucide-react'

import { FileDrop } from '@/components/file-drop'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { ImageWithFallback } from '@/components/image-with-fallback'
import { useDeleteStoreBanner } from '@/features/merchant/hooks/use-delete-store-banner'
import { useUploadStoreBanner } from '@/features/merchant/hooks/use-upload-store-banner'

const ACCEPTED = 'image/jpeg,image/png,image/webp'

/**
 * The store's banner: a wide preview of what customers see on their feed,
 * an upload/replace button, and a remove when there is one. Uploads post
 * immediately — the server resizes and stores the bytes; this card only
 * renders what came back.
 */
export function BannerCard({
  storeId,
  imageUrl,
}: {
  storeId: string
  imageUrl?: string | null
}) {
  const upload = useUploadStoreBanner()
  const remove = useDeleteStoreBanner()
  const fileInput = useRef<HTMLInputElement>(null)

  return (
    <Card className="overflow-hidden">
      <CardHeader className="border-b bg-muted/20">
        <CardTitle>Banner</CardTitle>
        <CardDescription>
          The wide image across the store&apos;s card in the customer app.
        </CardDescription>
      </CardHeader>
      <CardContent className="p-6">
        <FileDrop
          disabled={upload.isPending}
          onFiles={(files) => {
            const file = files[0]
            if (file) {
              upload.mutate({ path: { id: storeId }, body: { file } })
            }
          }}
        >
          <input
            ref={fileInput}
            type="file"
            accept={ACCEPTED}
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0]
              if (!file) return
              upload.mutate(
                { path: { id: storeId }, body: { file } },
                { onSettled: () => {
                  if (fileInput.current) fileInput.current.value = ''
                } },
              )
            }}
          />
          <div className="space-y-4">
            <div className="aspect-[21/9] w-full overflow-hidden rounded-xl border bg-muted/30">
              <ImageWithFallback
                src={imageUrl}
                alt="Store banner"
                className="h-full w-full object-cover"
              />
            </div>
            {!imageUrl && (
              <p className="text-center text-xs text-muted-foreground">
                Drop an image anywhere on this card, or use the button.
              </p>
            )}
            <div className="flex gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={upload.isPending}
                onClick={() => fileInput.current?.click()}
              >
                <ImagePlus data-icon="inline-start" />
                {upload.isPending
                  ? 'Uploading…'
                  : imageUrl
                    ? 'Replace banner'
                    : 'Upload banner'}
              </Button>
              {imageUrl && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground hover:text-destructive"
                  disabled={remove.isPending}
                  onClick={() => remove.mutate({ path: { id: storeId } })}
                >
                  <Trash2 data-icon="inline-start" />
                  Remove
                </Button>
              )}
            </div>
          </div>
        </FileDrop>
      </CardContent>
    </Card>
  )
}
