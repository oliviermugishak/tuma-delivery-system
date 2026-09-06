/**
 * The shared status vocabulary (constitution Part 6) — one component, no
 * forks. Status is a story: dot + text (P11), never color-only, never a
 * bare toggle. Every lifecycle maps to one of five tones:
 * accent (action/active/money) · success (live/healthy) · warning
 * (delay/pending) · danger (failure/destructive) · muted (off/idle).
 */

export type Tone = 'accent' | 'success' | 'warning' | 'danger' | 'muted'

/**
 * Order status — the six server values (AGENTS.md rule 8) plus the
 * derived "delayed" presentation, which the UI computes from a missed
 * promise and always renders with a revised promise (P11).
 */
/**
 * The server's `?status=` filter, as the merchant surface consumes it:
 * the Live feed (board, badge, overview, stores) wants the in-flight
 * statuses; History wants the settled ones. One fact, one place.
 */
export const LIVE_ORDER_STATUSES = 'placed,accepted,preparing,picked_up'
export const SETTLED_ORDER_STATUSES = 'delivered,cancelled'

export type OrderStatus =
  | 'placed'
  | 'accepted'
  | 'preparing'
  | 'picked_up'
  | 'delivered'
  | 'cancelled'

export function orderStatusTone(status: string): Tone {
  switch (status) {
    case 'placed':
      return 'warning'
    case 'accepted':
    case 'preparing':
    case 'picked_up':
      return 'accent'
    case 'delivered':
      return 'success'
    case 'cancelled':
      return 'danger'
    default:
      return 'muted'
  }
}

export function orderStatusLabel(status: string): string {
  switch (status) {
    case 'placed':
      return 'To accept'
    case 'accepted':
      return 'Accepted'
    case 'preparing':
      return 'Preparing'
    case 'picked_up':
      return 'Out for delivery'
    case 'delivered':
      return 'Delivered'
    case 'cancelled':
      return 'Cancelled'
    default:
      return status.replaceAll('_', ' ')
  }
}

/**
 * Merchant lifecycle: the server's two values (active/suspended) rendered
 * as the constitution's story. "Suspended" is the red terminal state.
 */
export function merchantStatusTone(status: string): Tone {
  switch (status) {
    case 'active':
      return 'success'
    case 'suspended':
      return 'danger'
    default:
      return 'muted'
  }
}

export function merchantStatusLabel(status: string): string {
  switch (status) {
    case 'active':
      return 'Active'
    case 'suspended':
      return 'Suspended'
    default:
      return status.replaceAll('_', ' ')
  }
}

/**
 * Cash payment state on a store order: `pending` (not yet collected),
 * `collected` (cash in hand), `refunded`. Cancelled orders surface this
 * as the cash-collection state (M2 receipt column).
 */
export function paymentStatusTone(status: string): Tone {
  switch (status) {
    case 'collected':
      return 'success'
    case 'refunded':
      return 'accent'
    default:
      return 'warning'
  }
}

export function paymentStatusLabel(status: string): string {
  switch (status) {
    case 'collected':
      return 'Cash · collected'
    case 'refunded':
      return 'Cash · refunded'
    case 'pending':
      return 'Cash · not collected'
    default:
      return `Cash · ${status.replaceAll('_', ' ')}`
  }
}

/**
 * Rider availability as the platform sees it: active riders are
 * assignable; inactive are off duty. (Live "On delivery" needs a
 * deliveries feed — no such endpoint exists yet.)
 */
export function riderStatusTone(isActive: boolean): Tone {
  return isActive ? 'success' : 'muted'
}

export function riderStatusLabel(isActive: boolean): string {
  return isActive ? 'Available' : 'Off duty'
}

/** Customer directory state (admin): Active / Deactivated. */
export function customerStatusTone(isActive: boolean): Tone {
  return isActive ? 'success' : 'danger'
}

export function customerStatusLabel(isActive: boolean): string {
  return isActive ? 'Active' : 'Deactivated'
}

/**
 * Store open state — dot + text beside the toggle (P11: a toggle is a
 * control, never the status itself).
 */
export function storeStatusTone(isOpen: boolean): Tone {
  return isOpen ? 'success' : 'muted'
}

export function storeStatusLabel(isOpen: boolean): string {
  return isOpen ? 'Open' : 'Closed'
}
