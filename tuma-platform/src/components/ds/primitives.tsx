/**
 * Tuma Phase-2 component kit — the one vocabulary (constitution P15).
 * Every screen composes from here; page-specific inventions are banned.
 * Visual ground truth: redesigns/dashboard_redesign.html. Tokens carry
 * all color; no raw hex appears below (P14). Zero shadows (P6).
 */
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'

import { cn } from '@/lib/utils'
import type { Tone } from '@/lib/status'

/* ------------------------------------------------------------------ */
/* Icon — Material Symbols Rounded. aria-label is mandatory (Part 5); */
/* the tooltip wrapper renders it on hover/focus.                      */
/* ------------------------------------------------------------------ */

export function Icon({
  name,
  label,
  size = 20,
  filled = false,
  className,
}: {
  name: string
  /** Spoken text + tooltip. Icons without a label are decorative: pass label="" */
  label?: string
  size?: number
  filled?: boolean
  className?: string
}) {
  const cls = cn(
    'ms',
    size === 16 && 's16',
    size === 18 && 's18',
    filled && 'f',
    className,
  )
  const style = size !== 16 && size !== 18 && size !== 20 ? { fontSize: size } : undefined
  if (label === undefined) {
    return (
      <span role="img" aria-label={name} className={cls} style={style}>
        {name}
      </span>
    )
  }
  if (label === '') {
    return (
      <span aria-hidden className={cls} style={style}>
        {name}
      </span>
    )
  }
  return (
    <span role="img" aria-label={label} title={label} className={cls} style={style}>
      {name}
    </span>
  )
}

/* ------------------------------------------------------------------ */
/* Status — dot + text (P11). Tones from lib/status.                   */
/* ------------------------------------------------------------------ */

const toneText: Record<Tone, string> = {
  accent: 'text-brand',
  success: 'text-success',
  warning: 'text-warning',
  danger: 'text-danger',
  muted: 'text-text2',
}

const toneDot: Record<Tone, string> = {
  accent: 'bg-brand',
  success: 'bg-success',
  warning: 'bg-warning',
  danger: 'bg-danger',
  muted: 'bg-text3',
}

export function Status({
  tone,
  children,
  small = false,
  className,
}: {
  tone: Tone
  children: ReactNode
  small?: boolean
  className?: string
}) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-2 font-semibold whitespace-nowrap',
        small ? 'text-[11.5px]' : 'text-[12.5px]',
        toneText[tone],
        className,
      )}
    >
      <i className={cn('h-2 w-2 shrink-0 rounded-full', toneDot[tone])} />
      {children}
    </span>
  )
}

/* The one live pulse per view (P13). */
export function Pulse({ warn = false }: { warn?: boolean }) {
  return (
    <span className={cn('pw', warn && 'warn')} aria-hidden>
      <i />
    </span>
  )
}

/** "Updated 30s ago" freshness label (P13). */
export function Freshness({
  updated,
  prefix,
  className,
}: {
  updated: string | null | undefined
  prefix?: string
  className?: string
}) {
  const [, force] = useState(0)
  useEffect(() => {
    const t = setInterval(() => force((n) => n + 1), 15_000)
    return () => clearInterval(t)
  }, [])
  const text = updated ? relTime(updated) : null
  if (!text) return null
  return (
    <span
      className={cn('inline-flex items-center gap-1.5 text-xs font-medium text-text3', className)}
    >
      {prefix ? `${prefix} · ` : ''}
      Updated {text}
    </span>
  )
}

function relTime(iso: string): string {
  const s = Math.max(0, Math.round((Date.now() - new Date(iso).getTime()) / 1000))
  if (s < 10) return 'just now'
  if (s < 60) return `${s}s ago`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m} min ago`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h}h ago`
  return `${Math.floor(h / 24)}d ago`
}

/* ------------------------------------------------------------------ */
/* Buttons — primary (accent, the one per view P5) / outline / ghost / */
/* danger-outline / danger-filled. h40 r14, sm h32.                    */
/* ------------------------------------------------------------------ */

const btnBase =
  'inline-flex items-center justify-center gap-2 h-10 px-4 rounded-[14px] text-[15px] font-semibold whitespace-nowrap border border-transparent transition-colors duration-150 disabled:opacity-50 disabled:cursor-not-allowed'

const btnVariants = {
  primary: 'bg-brand text-on-accent hover:brightness-106',
  outline: 'border-line text-foreground hover:bg-white/4',
  ghost: 'text-text2 hover:bg-white/4 hover:text-foreground',
  dangerOutline: 'border-danger/40 text-danger hover:bg-danger/8',
  dangerFilled: 'bg-danger text-on-accent hover:brightness-106',
}

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: keyof typeof btnVariants
  small?: boolean
}

export function Button({
  variant = 'outline',
  small = false,
  className,
  ...props
}: ButtonProps) {
  return (
    <button
      {...props}
      className={cn(
        btnBase,
        btnVariants[variant],
        small && 'h-8 px-3 rounded-[10px] text-[13px]',
        className,
      )}
    />
  )
}

/* ------------------------------------------------------------------ */
/* Chips — filter chips (fchip).                                        */
/* ------------------------------------------------------------------ */

export function Chip({
  on = false,
  onRemove,
  icon,
  children,
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  on?: boolean
  onRemove?: () => void
  icon?: string
}) {
  return (
    <button
      {...props}
      className={cn(
        'inline-flex h-8 items-center gap-1.5 rounded-full border px-3 text-[13px] font-medium whitespace-nowrap transition-colors duration-150',
        on
          ? 'border-brand/45 bg-brand/7 text-brand'
          : 'border-line text-text2 hover:text-foreground',
        className,
      )}
    >
      {icon ? <Icon name={icon} label="" size={16} /> : null}
      {children}
      {onRemove ? (
        <span
          role="button"
          tabIndex={0}
          aria-label="Remove filter"
          onClick={(e) => {
            e.stopPropagation()
            onRemove()
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.stopPropagation()
              onRemove()
            }
          }}
          className="-mr-1 grid size-4 place-items-center rounded-full hover:text-foreground"
        >
          <Icon name="close" label="" size={16} />
        </span>
      ) : null}
    </button>
  )
}

/* ------------------------------------------------------------------ */
/* Cards / KPI (Part 5 anatomy: micro-label, tabular value, delta by    */
/* good/bad, optional sparkline in the viz palette).                    */
/* ------------------------------------------------------------------ */

export function Card({
  className,
  children,
  tone,
}: {
  className?: string
  children: ReactNode
  tone?: 'danger'
}) {
  return (
    <div
      className={cn(
        'rounded-2xl border border-white/8 bg-surface p-6',
        tone === 'danger' && 'border-danger/28',
        className,
      )}
    >
      {children}
    </div>
  )
}

export function MicroLabel({
  children,
  className,
}: {
  children: ReactNode
  className?: string
}) {
  return (
    <div
      className={cn(
        'text-[11px] font-semibold tracking-[0.09em] uppercase text-text2',
        className,
      )}
    >
      {children}
    </div>
  )
}

/** Delta chip colored by good/bad, never by direction (P14). */
export function Delta({
  good,
  children,
}: {
  good: boolean
  children: ReactNode
}) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-bold',
        good ? 'bg-success/10 text-success' : 'bg-danger/10 text-danger',
      )}
    >
      {children}
    </span>
  )
}

export function KpiCard({
  label,
  value,
  unit,
  delta,
  deltaGood,
  vs,
  spark,
}: {
  label: string
  value: string
  unit?: string
  delta?: string
  deltaGood?: boolean
  vs?: string
  /** 0..1 series, rendered in the given viz color. */
  spark?: { points: number[]; color: string }
}) {
  return (
    <div className="rounded-2xl border border-white/8 bg-surface p-5">
      <MicroLabel>{label}</MicroLabel>
      <div className="mt-2.5 flex items-baseline gap-1.5 text-[30px] leading-none font-extrabold tracking-tight">
        <span>{value}</span>
        {unit ? (
          <span className="text-sm font-semibold text-text2">{unit}</span>
        ) : null}
      </div>
      {delta || spark ? (
        <div className="mt-2 flex items-center gap-2">
          {delta ? (
            <Delta good={deltaGood === true}>{delta}</Delta>
          ) : null}
          {vs ? <span className="text-[11.5px] text-text3">{vs}</span> : null}
          {spark && spark.points.length > 1 ? (
            <Sparkline
              points={spark.points}
              color={spark.color}
              className="ml-auto"
            />
          ) : null}
        </div>
      ) : null}
    </div>
  )
}

const VIZ: Record<string, string> = {
  accent: 'var(--viz1)',
  success: 'var(--viz2)',
  blue: 'var(--viz3)',
  pink: 'var(--viz4)',
  lime: 'var(--viz5)',
}

/** Flat thin sparkline — the ≤10%-alpha fill is sanctioned (P14). */
export function Sparkline({
  points,
  color,
  className,
}: {
  points: number[]
  color: keyof typeof VIZ | string
  className?: string
}) {
  const stroke = VIZ[color] ?? color
  const w = 64
  const h = 24
  const min = Math.min(...points)
  const max = Math.max(...points)
  const span = max - min || 1
  const step = (w - 4) / (points.length - 1)
  const coords = points
    .map((p, i) => `${2 + i * step},${h - 4 - ((p - min) / span) * (h - 8)}`)
    .join(' ')
  return (
    <svg width={w} height={h} className={className} aria-hidden>
      <polyline
        points={coords}
        fill="none"
        stroke={stroke}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

/* ------------------------------------------------------------------ */
/* Inputs — label above, inline validation (on blur/submit only),       */
/* error border. Used by all forms and the dirty-save flow.             */
/* ------------------------------------------------------------------ */

export function Field({
  label,
  error,
  help,
  children,
  className,
}: {
  label: string
  error?: string | null
  help?: string
  children: ReactNode
  className?: string
}) {
  return (
    <label className={cn('flex flex-col gap-1.5', className)}>
      <span className="text-[13px] font-medium text-text2">{label}</span>
      {children}
      {error ? (
        <span className="text-xs font-medium text-danger">{error}</span>
      ) : help ? (
        <span className="text-xs text-text3">{help}</span>
      ) : null}
    </label>
  )
}

export const inputCls =
  'h-10 w-full rounded-xl border border-line bg-high px-3 text-sm text-foreground placeholder:text-text3 focus:border-text3 focus:outline-none'

export function Input(
  props: React.InputHTMLAttributes<HTMLInputElement> & { invalid?: boolean },
) {
  const { invalid, className, ...rest } = props
  return (
    <input
      {...rest}
      className={cn(inputCls, invalid && 'border-danger', className)}
    />
  )
}

export function Textarea(
  props: React.TextareaHTMLAttributes<HTMLTextAreaElement> & {
    invalid?: boolean
  },
) {
  const { invalid, className, ...rest } = props
  return (
    <textarea
      {...rest}
      className={cn(
        inputCls,
        'h-auto resize-none py-2.5',
        invalid && 'border-danger',
        className,
      )}
    />
  )
}

export function Select(
  props: React.SelectHTMLAttributes<HTMLSelectElement> & { invalid?: boolean },
) {
  const { invalid, className, children, ...rest } = props
  return (
    <div className="relative">
      <select
        {...rest}
        className={cn(
          inputCls,
          'appearance-none pr-9',
          invalid && 'border-danger',
          className,
        )}
      >
        {children}
      </select>
      <Icon
        name="expand_more"
        label=""
        size={18}
        className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-text3"
      />
    </div>
  )
}

/* Toggle — always paired with dot+text status by the caller (P11). */
export function Toggle({
  on,
  onChange,
  label,
  disabled,
}: {
  on: boolean
  onChange: (next: boolean) => void
  label: string
  disabled?: boolean
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!on)}
      className={cn(
        'relative h-5 w-9 shrink-0 rounded-full border transition-colors duration-150',
        on ? 'border-brand bg-brand' : 'border-line bg-high',
        disabled && 'cursor-not-allowed opacity-50',
      )}
    >
      <span
        className={cn(
          'absolute top-0.5 size-3.5 rounded-full transition-[left] duration-150',
          on ? 'left-[18px] bg-on-accent' : 'left-0.5 bg-text3',
        )}
      />
    </button>
  )
}

/* ------------------------------------------------------------------ */
/* Copyable ID — "#1042", click to copy (Part 5).                       */
/* ------------------------------------------------------------------ */

export function CopyableId({
  id,
  className,
}: {
  id: string | number
  className?: string
}) {
  const [copied, setCopied] = useState(false)
  return (
    <button
      type="button"
      title={`Copy #${id}`}
      aria-label={`Copy ID #${id}`}
      onClick={() => {
        void navigator.clipboard?.writeText(String(id)).then(
          () => {
            setCopied(true)
            setTimeout(() => setCopied(false), 1200)
          },
          () => undefined,
        )
      }}
      className={cn(
        'inline-flex items-center gap-1 font-semibold text-text2 hover:text-foreground',
        className,
      )}
    >
      #{id}
      <Icon
        name={copied ? 'check' : 'content_copy'}
        label=""
        size={14}
        className="text-text3"
      />
    </button>
  )
}

/* ------------------------------------------------------------------ */
/* Avatar — image with initials fallback.                               */
/* ------------------------------------------------------------------ */

export function Avatar({
  text,
  src,
  large = false,
  className,
}: {
  text: string
  src?: string | null
  large?: boolean
  className?: string
}) {
  return (
    <span
      className={cn(
        'grid shrink-0 place-items-center rounded-full bg-high font-bold text-brand',
        large ? 'size-10 text-sm' : 'size-8 text-xs',
        className,
      )}
    >
      {src ? (
        <img src={src} alt="" className="size-full rounded-full object-cover" />
      ) : (
        text
      )}
    </span>
  )
}

/* ------------------------------------------------------------------ */
/* Kebab menu (popover) — the row-actions vocabulary. Items are         */
/* {label, icon, onSelect, danger?}.                                    */
/* ------------------------------------------------------------------ */

export function Kebab({
  items,
  label = 'Row actions',
}: {
  items: Array<{
    label: string
    icon: string
    onSelect: () => void
    danger?: boolean
  }>
  label?: string
}) {
  const [open, setOpen] = useState(false)
  const btnRef = useRef<HTMLButtonElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null)

  // The menu renders in a body portal at fixed coordinates so the table's
  // overflow containers can't clip it and it stays clickable above drawers.
  const openMenu = () => {
    const r = btnRef.current?.getBoundingClientRect()
    if (r) {
      setPos({
        top: r.bottom + 6,
        left: Math.max(8, r.right - 200),
      })
    }
    setOpen(true)
  }

  useEffect(() => {
    if (!open) return
    const onDoc = (e: MouseEvent) => {
      const t = e.target as Node
      if (!btnRef.current?.contains(t) && !menuRef.current?.contains(t))
        setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false)
    }
    const onScrollOrResize = () => setOpen(false)
    document.addEventListener('mousedown', onDoc)
    document.addEventListener('keydown', onKey)
    window.addEventListener('scroll', onScrollOrResize, true)
    window.addEventListener('resize', onScrollOrResize)
    return () => {
      document.removeEventListener('mousedown', onDoc)
      document.removeEventListener('keydown', onKey)
      window.removeEventListener('scroll', onScrollOrResize, true)
      window.removeEventListener('resize', onScrollOrResize)
    }
  }, [open])

  return (
    <>
      <button
        ref={btnRef}
        type="button"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={(e) => {
          // Inside a clickable table row: never let the kebab open the row.
          e.stopPropagation()
          open ? setOpen(false) : openMenu()
        }}
        className="grid size-9 place-items-center rounded-[10px] text-text2 hover:bg-white/4 hover:text-foreground"
      >
        <Icon name="more_vert" label="" size={18} />
      </button>
      {open && pos
        ? createPortal(
            <div
              ref={menuRef}
              role="menu"
              style={{ top: pos.top, left: pos.left }}
              className="fixed z-[70] min-w-50 rounded-xl border border-white/8 bg-high p-1.5 text-left"
            >
              {items.map((item) => (
                <button
                  key={item.label}
                  role="menuitem"
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation()
                    setOpen(false)
                    item.onSelect()
                  }}
                  className={cn(
                    'flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13.5px] font-medium text-text2 hover:bg-white/4 hover:text-foreground',
                    item.danger && 'text-danger hover:text-danger',
                  )}
                >
                  <Icon name={item.icon} label="" size={17} />
                  {item.label}
                </button>
              ))}
            </div>,
            document.body,
          )
        : null}
    </>
  )
}

/* ------------------------------------------------------------------ */
/* Empty / Error states (P12, P17): icon + one line + the next action.  */
/* ------------------------------------------------------------------ */

export function EmptyState({
  icon,
  title,
  hint,
  action,
  className,
}: {
  icon: string
  title: string
  hint?: string
  action?: ReactNode
  className?: string
}) {
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center gap-1.5 px-6 py-14 text-center',
        className,
      )}
    >
      <Icon name={icon} label="" size={36} className="text-text3" />
      <div className="mt-2 text-[15px] font-bold">{title}</div>
      {hint ? <div className="text-[13px] text-text2">{hint}</div> : null}
      {action ? <div className="mt-3">{action}</div> : null}
    </div>
  )
}

export function ErrorState({
  onRetry,
  title = 'Couldn’t load this view',
  hint = 'Something went wrong while reaching the server.',
}: {
  onRetry: () => void
  title?: string
  hint?: string
}) {
  return (
    <EmptyState
      icon="wifi_off"
      title={title}
      hint={hint}
      action={
        <Button small onClick={onRetry}>
          <Icon name="refresh" label="" size={16} />
          Try again
        </Button>
      }
    />
  )
}

/* Skeleton — shimmer matches the final layout (Part 5). */
export function Skeleton({ className }: { className?: string }) {
  return <div className={cn('tuma-sk', className)} />
}

export function TableSkeleton({ rows = 8 }: { rows?: number }) {
  return (
    <div className="space-y-px" aria-hidden>
      <Skeleton className="h-11 w-full rounded-none" />
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="flex items-center gap-4 px-4 py-3.5">
          <Skeleton className="h-4 w-14" />
          <Skeleton className="h-4 w-40" />
          <Skeleton className="ml-auto h-4 w-16" />
          <Skeleton className="h-4 w-20" />
        </div>
      ))}
    </div>
  )
}

export function KpiSkeleton() {
  return (
    <div className="rounded-2xl border border-white/8 bg-surface p-5">
      <Skeleton className="h-3 w-24" />
      <Skeleton className="mt-4 h-8 w-28" />
      <Skeleton className="mt-3 h-4 w-36" />
    </div>
  )
}

/* ------------------------------------------------------------------ */
/* Page scaffolding — pagehead, toolbar, section titles.                */
/* ------------------------------------------------------------------ */

export function PageHead({
  title,
  sub,
  actions,
  back,
}: {
  title: ReactNode
  sub?: ReactNode
  actions?: ReactNode
  back?: { to: string; label: string }
}) {
  return (
    <div>
      {back ? (
        <a
          href={back.to}
          className="mb-2 inline-flex items-center gap-1.5 text-[13px] font-semibold text-text2 hover:text-foreground"
        >
          <Icon name="arrow_back" label="" size={16} />
          {back.label}
        </a>
      ) : null}
      <div className="flex flex-wrap items-start gap-6">
        <div className="min-w-0 flex-1">
          <h1 className="text-[22px] font-extrabold tracking-tight">{title}</h1>
          {sub ? (
            <div className="mt-1 max-w-2xl text-sm text-text2">{sub}</div>
          ) : null}
        </div>
        {actions ? (
          <div className="flex items-center gap-2.5">{actions}</div>
        ) : null}
      </div>
    </div>
  )
}

/** Row of facts (label left, value right) used in cards/drawers. */
export function FactRow({
  label,
  children,
  accent = false,
}: {
  label: ReactNode
  children: ReactNode
  accent?: boolean
}) {
  return (
    <div className="flex items-center gap-3 border-t border-white/8 py-2.5 text-[13.5px] first:border-t-0">
      <span className="flex-1 text-text2">{label}</span>
      <span
        className={cn(
          'font-semibold',
          accent ? 'text-brand' : 'text-foreground',
        )}
      >
        {children}
      </span>
    </div>
  )
}
