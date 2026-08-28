/** Format an ISO-8601 timestamp as a short human date, e.g. "28 Aug 2026". */
export function formatDate(iso: string | null | undefined): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return '—'
  return date.toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  })
}

/** Initials for an avatar: from the name if there is one, else the email. */
export function initials(
  name?: string | null,
  email?: string | null,
): string {
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
 * Integer RWF as people see it: "3,500 RWF". Money on Tuma is whole francs,
 * server-owned — this only formats, never computes.
 */
export function formatRwf(amount: number): string {
  return `${amount.toLocaleString('en-US')} RWF`
}
