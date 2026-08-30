import { useState, type DragEvent, type ReactNode } from 'react'

import { cn } from '@/lib/utils'

/**
 * Drag & drop for image uploads — native HTML5, no dependency. Wraps any
 * upload surface (gallery editor, banner card, create-form picker);
 * children render as-is, a dashed overlay covers the area while a drag
 * is in flight. Only image files survive the drop; click-to-browse stays
 * whatever the children provide.
 */
export function FileDrop({
  onFiles,
  disabled = false,
  className,
  children,
}: {
  onFiles: (files: File[]) => void
  disabled?: boolean
  className?: string
  children: ReactNode
}) {
  const [dragging, setDragging] = useState(false)

  const onDragOver = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault()
    if (!disabled) setDragging(true)
  }

  const onDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault()
    setDragging(false)
    if (disabled) return
    const files = Array.from(event.dataTransfer.files).filter((file) =>
      file.type.startsWith('image/'),
    )
    if (files.length > 0) onFiles(files)
  }

  return (
    <div
      className={cn('relative', className)}
      onDragOver={onDragOver}
      onDragLeave={() => setDragging(false)}
      onDrop={onDrop}
    >
      {children}
      {dragging && (
        <div
          className="absolute inset-0 z-10 flex items-center justify-center rounded-xl border-2 border-dashed border-primary bg-background/85 text-sm font-medium text-primary"
          aria-hidden
        >
          Drop images to upload
        </div>
      )}
    </div>
  )
}
