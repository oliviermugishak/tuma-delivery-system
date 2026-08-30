import { Package } from 'lucide-react'

/**
 * The one rule for rendering merchandise imagery: a real image when there
 * is one (uploaded banner / gallery cover / external URL), otherwise the
 * quiet Package block. Fills its parent — cards give it an aspect box.
 */
export function ImageWithFallback({
  src,
  alt,
  className = 'h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.03]',
}: {
  src?: string | null
  alt: string
  className?: string
}) {
  if (!src) {
    return (
      <div className="flex h-full w-full items-center justify-center">
        <Package className="size-8 text-muted-foreground/40" aria-hidden />
      </div>
    )
  }
  return <img src={src} alt={alt} loading="lazy" className={className} />
}
