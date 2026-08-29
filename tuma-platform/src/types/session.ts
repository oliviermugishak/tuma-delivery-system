import type { MeResponse } from '@/api/generated'

/**
 * The session account as returned by GET /v1/me. The account is roleless —
 * what it can do is visible in its profiles and memberships:
 *   - `admin` set            → platform control plane
 *   - `merchant_memberships` → merchant wing (per business, optionally
 *     store-scoped)
 *   - `customer` only        → the mobile app is their surface, not this
 *     platform.
 */
export type SessionUser = MeResponse

export function isPlatformAdmin(user: SessionUser | null | undefined): boolean {
  return user?.admin != null
}

/** The account's merchant memberships (possibly several, one per business). */
export function merchantMemberships(user: SessionUser | null | undefined) {
  return user?.merchant_memberships ?? []
}

/** Whether the account can enter the merchant wing at all. */
export function isMerchantOperator(
  user: SessionUser | null | undefined,
): boolean {
  return merchantMemberships(user).length > 0
}

/**
 * The display name across profiles — the customer/admin profile name when
 * present, else the email, else a fallback. Merchant operators may have no
 * editable profile; their wing identity is the business.
 */
export function displayName(user: SessionUser | null | undefined): string {
  const name = user?.customer?.name || user?.admin?.name
  if (name) return name
  return user?.email || 'Signed in'
}
