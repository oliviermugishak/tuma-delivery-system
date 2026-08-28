import type { MeResponse } from '@/api/generated'

/** V1 roles carried in the JWT and returned by /v1/me. */
export type Role = 'admin' | 'merchant' | 'customer'

/** The session user as returned by GET /v1/me. */
export type SessionUser = MeResponse
