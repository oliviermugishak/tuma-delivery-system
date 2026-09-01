/**
 * Local record of hand-off assignments — the operator's own input.
 *
 * Why this exists: the server's fulfillment-sheet response
 * (GET /v1/merchant/store-orders/{id}) carries no rider identity — the
 * delivery row has rider_id but the response never joins it (see
 * tuma-platform/INTEGRATION-NOTES.md, finding 1). Until that additive
 * server field lands, the "who did we hand it to" answer lives here:
 * the rider number the operator actually typed at hand-off, stored per
 * store order. Nothing is invented — a number shows only if this
 * operator entered it on this device.
 */

const KEY = 'tuma.handoff.log.v1'

type Log = Record<string, number>

function read(): Log {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? '{}') as Log
  } catch {
    return {}
  }
}

export function recordHandoff(storeOrderId: string, riderNumber: number) {
  try {
    const log = read()
    log[storeOrderId] = riderNumber
    localStorage.setItem(KEY, JSON.stringify(log))
  } catch {
    // Storage unavailable — the assignment itself already succeeded
    // server-side; only the local display is lost.
  }
}

export function lookupHandoff(storeOrderId: string): number | null {
  return read()[storeOrderId] ?? null
}
