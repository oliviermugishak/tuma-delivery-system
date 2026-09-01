/**
 * ══════════════════════════════════════════════════════════════════
 *  DEMO SEED — client-only rows for screens whose server endpoints do
 *  not exist yet. Every usage is marked `BACKEND GAP` in the page code
 *  and catalogued in tuma-platform/BACKEND-GAPS.md with the exact
 *  endpoint contract the server needs. Mutations against demo rows stay
 *  in client state and never persist — the server owns truth.
 *  When a listed endpoint lands, its page swaps the demo hook for the
 *  generated query and this file shrinks.
 * ══════════════════════════════════════════════════════════════════
 */

import type { Tone } from '@/lib/status'

/*
 * Founder decision (2026-09-01): admins never see orders — merchants own
 * the order lifecycle, the admin wing manages merchants, riders,
 * customers, and the platform. The old G1 admin orders feed / G2 admin
 * refund demo rows are gone with the admin Orders page; refunds surface
 * through disputes (G3).
 */

/* G3 — disputes & refunds queue: GET /v1/admin/disputes (+ resolve). */
export interface DemoDispute {
  id: string
  number: number
  orderNumber: number
  store: string
  customer: string
  openedAt: string
  amount: number
  status: 'open' | 'resolved'
  reason: string
  events: Array<{ at: string; actor: string; text: string }>
}

export const demoDisputes: DemoDispute[] = [
  {
    id: 'd1',
    number: 18,
    orderNumber: 1031,
    store: 'Java House City Center',
    customer: 'Clarisse Uwimana',
    openedAt: '2026-08-26T18:20:00Z',
    amount: 12000,
    status: 'open',
    reason: 'Order never arrived — customer waited past the promise twice.',
    events: [
      { at: '2026-08-26T18:20:00Z', actor: 'Clarisse Uwimana', text: 'Opened the dispute: order never arrived.' },
      { at: '2026-08-26T18:41:00Z', actor: 'Tuma Admin', text: 'Asked the store for the hand-off confirmation.' },
      { at: '2026-08-26T19:02:00Z', actor: 'Java House City Center', text: 'Confirmed the rider never collected the order.' },
    ],
  },
  {
    id: 'd2',
    number: 17,
    orderNumber: 1027,
    store: 'Simba Remera',
    customer: 'Aline Uwase',
    openedAt: '2026-08-26T14:05:00Z',
    amount: 12000,
    status: 'open',
    reason: 'One item arrived damaged (rice bag torn open).',
    events: [
      { at: '2026-08-26T14:05:00Z', actor: 'Aline Uwase', text: 'Opened the dispute: item arrived damaged.' },
      { at: '2026-08-26T15:10:00Z', actor: 'Tuma Admin', text: 'Requested a photo from the customer.' },
    ],
  },
  {
    id: 'd3',
    number: 16,
    orderNumber: 1019,
    store: 'KFC Remera',
    customer: 'Eric Nshimiyimana',
    openedAt: '2026-08-25T13:12:00Z',
    amount: 8500,
    status: 'resolved',
    reason: 'Wrong drink delivered; customer accepted a partial refund.',
    events: [
      { at: '2026-08-25T13:12:00Z', actor: 'Eric Nshimiyimana', text: 'Opened the dispute: wrong drink.' },
      { at: '2026-08-25T16:40:00Z', actor: 'Tuma Admin', text: 'Resolved with a 1,000 RWF partial refund.' },
    ],
  },
]

/* G4 — merchant payouts (Earnings page gap-shower): the ADMIN payout
   queue was removed — admins don't act on payouts in the current server
   surface; the merchant-side earnings page keeps its demo rows until the
   payments milestone (G4). */
export interface DemoPayout {
  id: string
  merchant: string
  amount: number
  requestedAt: string
  status: 'pending' | 'processing' | 'paid' | 'failed'
  period: string
  failureReason?: string
}

export const demoMerchantPayouts: DemoPayout[] = [
  {
    id: 'm47',
    merchant: 'KFC Rwanda',
    amount: 96000,
    requestedAt: '2026-08-23T09:00:00Z',
    status: 'paid',
    period: '13 – 19 Aug',
  },
  {
    id: 'm46',
    merchant: 'KFC Rwanda',
    amount: 88000,
    requestedAt: '2026-08-16T09:00:00Z',
    status: 'paid',
    period: '6 – 12 Aug',
  },
  {
    id: 'm45',
    merchant: 'KFC Rwanda',
    amount: 72500,
    requestedAt: '2026-08-09T09:00:00Z',
    status: 'failed',
    period: '30 Jul – 5 Aug',
    failureReason: 'Bank rejected the account number — update the payout details.',
  },
]

/* G5 (fees) and G6 (audit) were retired by founder direction — the pages
   were removed; when the platform needs them again the endpoints land as
   normal slices. */

/* G7 — reviews: GET /v1/merchant/reviews (+ reply). */
export interface DemoReview {
  id: string
  orderNumber: number
  customer: string
  rating: number
  comment: string
  at: string
  reply: string | null
}

export const demoReviews: DemoReview[] = [
  { id: 'r3', orderNumber: 1039, customer: 'Diane Mukamana', rating: 5, comment: 'Hot food, exactly on time. The delivery updates were great.', at: '2026-08-27T12:40:00Z', reply: null },
  { id: 'r2', orderNumber: 1036, customer: 'Aline Uwase', rating: 4, comment: 'Good chicken, fries were a bit cold by arrival.', at: '2026-08-27T10:31:00Z', reply: 'Thank you — we’re working on packing that keeps the fries crisp.' },
  { id: 'r1', orderNumber: 1028, customer: 'Jean-Paul Habimana', rating: 2, comment: 'Waited 20 extra minutes and no one told me why.', at: '2026-08-26T19:02:00Z', reply: null },
]

/* G8 — merchant onboarding states: the server only has active/suspended
   (02_marketplace.sql); "pending review" needs a migration. The old
   pending-review demo row was removed — the admin wing now shows the two
   real states only. */

/* G9 — revenue history: GET /v1/admin/metrics?range=7d (chart + deltas). */
export const demoRevenue7d: Array<{ date: string; value: number }> = [
  { date: '21 Aug', value: 512000 },
  { date: '22 Aug', value: 578000 },
  { date: '23 Aug', value: 501000 },
  { date: '24 Aug', value: 693000 },
  { date: '25 Aug', value: 655000 },
  { date: '26 Aug', value: 922000 },
  { date: '27 Aug', value: 951000 },
]

export const demoMerchantOrders7d: Array<{ date: string; value: number }> = [
  { date: '21', value: 9 },
  { date: '22', value: 11 },
  { date: '23', value: 8 },
  { date: '24', value: 14 },
  { date: '25', value: 12 },
  { date: '26', value: 16 },
  { date: '27', value: 12 },
]

/* G10 — rider live status & delivery stats: GET /v1/admin/riders/stats. */
export const demoRiderStats: Record<string, { deliveriesToday: number; onTimePct: number; onDelivery: boolean }> = {
  '2': { deliveriesToday: 6, onTimePct: 94, onDelivery: true },
  '3': { deliveriesToday: 4, onTimePct: 100, onDelivery: false },
  '4': { deliveriesToday: 7, onTimePct: 88, onDelivery: true },
}

export function payoutTone(status: DemoPayout['status']): Tone {
  switch (status) {
    case 'paid':
      return 'success'
    case 'pending':
    case 'processing':
      return 'accent'
    case 'failed':
      return 'danger'
  }
}

export function payoutLabel(status: DemoPayout['status']): string {
  switch (status) {
    case 'paid':
      return 'Paid'
    case 'pending':
      return 'Pending'
    case 'processing':
      return 'Processing'
    case 'failed':
      return 'Failed'
  }
}
