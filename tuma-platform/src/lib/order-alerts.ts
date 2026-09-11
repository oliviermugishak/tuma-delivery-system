/**
 * New-order alerting for the merchant wing — local-only, no server round
 * trip. Three honest channels, each fail-silent:
 * - an in-app toast (the board's poll detects the arrival),
 * - a WebAudio ping (mutable; two synthesized notes, no audio assets),
 * - a Notification pop-up (only if the operator granted permission —
 *   asked exactly once, from the bell's explicit button).
 *
 * Same conventions as handoff-log.ts: module-level functions, a
 * namespaced localStorage key, JSON, try/catch that never throws —
 * alerting must never break the board that renders the orders.
 */

const KEY = 'tuma.notify.v1'

type Prefs = { muted: boolean }

function readPrefs(): Prefs {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? '{}') as Prefs
  } catch {
    return { muted: false }
  }
}

/** True when the operator turned the new-order sound off. */
export function isMuted(): boolean {
  return readPrefs().muted === true
}

export function setMuted(muted: boolean): void {
  try {
    localStorage.setItem(KEY, JSON.stringify({ muted } satisfies Prefs))
  } catch {
    // Storage unavailable — only the preference is lost, nothing breaks.
  }
}

/**
 * Two short synthesized sine notes (880 Hz then 1320 Hz) through WebAudio,
 * each with a gain envelope to zero. No audio assets, no dependencies. The
 * whole body is fail-silent: an autoplay policy block or missing support
 * is fine — the toast already announced the order.
 */
export function playPing(): void {
  try {
    const ctx = new AudioContext()
    void ctx.resume()
    const now = ctx.currentTime
    const note = (at: number, freq: number) => {
      const osc = ctx.createOscillator()
      const gain = ctx.createGain()
      osc.type = 'sine'
      osc.frequency.value = freq
      gain.gain.setValueAtTime(0, at)
      gain.gain.linearRampToValueAtTime(0.18, at + 0.02)
      gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.12)
      osc.connect(gain).connect(ctx.destination)
      osc.start(at)
      osc.stop(at + 0.13)
    }
    note(now, 880)
    note(now + 0.13, 1320)
    // Release the audio hardware once both notes have sounded.
    window.setTimeout(() => void ctx.close().catch(() => {}), 400)
  } catch {
    // No WebAudio, or the autoplay policy refused — a silent failure is
    // fine.
  }
}

/**
 * A desktop pop-up — only when the operator has already granted
 * Notification permission. Never requests permission itself: that is the
 * bell's explicit button's job, asked once, never nagged.
 */
export function notifyOrder(title: string, body: string): void {
  try {
    if (
      typeof Notification !== 'undefined' &&
      Notification.permission === 'granted'
    ) {
      new Notification(title, { body })
    }
  } catch {
    // Notifications unavailable or blocked — a silent failure is fine.
  }
}

/**
 * PURE: which order ids are at status `placed` without having been seen
 * before, in encounter order. The caller owns the seen-set lifecycle
 * (seed everything on first load, merge after each pass) so this stays
 * trivially testable.
 */
export function placedArrivals(
  orders: { id: string; status: string }[],
  seen: ReadonlySet<string>,
): string[] {
  const arrivals: string[] = []
  for (const order of orders) {
    if (order.status === 'placed' && !seen.has(order.id)) {
      arrivals.push(order.id)
    }
  }
  return arrivals
}
