/**
 * The one formatting module (constitution Part 10: one money formatter,
 * one date formatter — no exceptions). All money is integer RWF owned by
 * the server; this only formats, never computes.
 */

/** "8,500" — the numerals only, tabular by the global font-feature. */
export function num(value: number | null | undefined): string {
  if (value == null) return ''
  return value.toLocaleString('en-US')
}

/** "8,500 RWF" — money at the point of decision (P8). */
export function rwf(amount: number | null | undefined): string {
  if (amount == null) return ''
  return `${num(amount)} RWF`
}

/**
 * Absolute date+time as the vocabulary demands: "27 Aug, 12:31".
 * Dates are absolute; relative ("2 min ago") is reserved for live feeds
 * and lives in <Freshness>.
 */
export function dateTime(iso: string | null | undefined): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  return d.toLocaleDateString('en-GB', { day: 'numeric', month: 'short' }) +
    ', ' +
    d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', hour12: false })
}

/** Absolute date only: "27 Aug 2026". */
export function date(iso: string | null | undefined): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  return d.toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  })
}

/** Time only: "12:31". */
export function time(iso: string | null | undefined): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  return d.toLocaleTimeString('en-GB', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}

/** "+250 788 004 512" spacing for raw phone digits (no + assumed). */
export function phone(raw: string | null | undefined): string {
  if (!raw) return ''
  const digits = raw.replace(/[^\d+]/g, '')
  if (digits.length < 9) return raw
  const tail = digits.slice(-9)
  const head = digits.slice(0, digits.length - 9)
  return `${head} ${tail.slice(0, 3)} ${tail.slice(3, 6)} ${tail.slice(6)}`.trim()
}

/** "TA" — two-letter avatar initials from a name, else from an email. */
export function initials(name?: string | null, email?: string | null): string {
  const fromName = (name ?? '')
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part.charAt(0).toUpperCase())
    .join('')
  if (fromName) return fromName
  return (email ?? '').slice(0, 2).toUpperCase() || '?'
}

/**
 * "Updated 30s ago" — relative time for live feeds only (P13).
 * Ticks at minute granularity.
 */
export function ago(iso: string | null | undefined): string {
  if (!iso) return ''
  const then = new Date(iso).getTime()
  if (Number.isNaN(then)) return ''
  const s = Math.max(0, Math.round((Date.now() - then) / 1000))
  if (s < 10) return 'just now'
  if (s < 60) return `${s}s ago`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m} min ago`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h}h ago`
  return `${Math.floor(h / 24)}d ago`
}

/**
 * "16,500" with the amount as the verb's object: returns the string used
 * by CTAs that carry money ("Accept · 16,500 RWF").
 */
export function ctaMoney(amount: number): string {
  return `${num(amount)} RWF`
}
