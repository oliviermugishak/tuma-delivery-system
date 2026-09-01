/**
 * Guard confirm dialog (P10) — the ONLY centered dialog allowed (P18's
 * exhaustive overlay list). Small, r20, restates the consequence and the
 * money. Every irreversible action routes through this component.
 */
import { useEffect, useRef, type ReactNode } from 'react'

import { cn } from '@/lib/utils'
import { Button, Icon } from './primitives'

export function GuardDialog({
  open,
  onClose,
  title,
  children,
  note,
  confirmLabel,
  onConfirm,
  pending = false,
  danger = true,
}: {
  open: boolean
  onClose: () => void
  title: string
  /** Consequence + amounts, bold-marked. */
  children: ReactNode
  /** The amber info note ("This can't be undone." / safe alternative). */
  note?: ReactNode
  confirmLabel: string
  onConfirm: () => void
  pending?: boolean
  danger?: boolean
}) {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [open, onClose])

  if (!open) return null
  return (
    <div className="fixed inset-0 z-50 grid place-items-center p-4">
      <div
        className="absolute inset-0 bg-[rgba(3,7,17,0.55)]"
        onClick={onClose}
        aria-hidden
      />
      <div
        ref={ref}
        role="alertdialog"
        aria-modal="true"
        aria-label={title}
        className={cn(
          'relative w-full max-w-125 rounded-[20px] border border-white/8 bg-surface p-6',
        )}
      >
        <div className="flex items-start gap-3">
          <h2 className="flex-1 pr-10 text-[17px] font-bold">{title}</h2>
          <button
            type="button"
            aria-label="Close dialog"
            onClick={onClose}
            className="grid size-9 place-items-center rounded-[10px] text-text2 hover:bg-white/4 hover:text-foreground"
          >
            <Icon name="close" label="" size={18} />
          </button>
        </div>
        <div className="mt-2.5 mb-4 text-sm leading-relaxed text-text2 [&_b]:font-bold [&_b]:text-foreground">
          {children}
        </div>
        {note ? (
          <div className="mt-3.5 flex items-start gap-2 rounded-[10px] bg-warning/8 px-3 py-2.5 text-[12.5px] font-medium text-warning">
            <Icon name="info" label="" size={16} className="mt-0.5" />
            <div>{note}</div>
          </div>
        ) : null}
        <div className="mt-5 flex justify-end gap-3">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant={danger ? 'dangerFilled' : 'primary'}
            onClick={onConfirm}
            disabled={pending}
          >
            {pending ? 'Working…' : confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  )
}
