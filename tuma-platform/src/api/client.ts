/**
 * Hand-written wrapper around the hey-api generated client. It owns the
 * three cross-cutting transport concerns for the platform; the generated
 * code in ./generated stays untouched.
 *
 *  1. cookie credentials — the session lives in httpOnly cookies, so every
 *     request must carry `credentials: 'include'`;
 *  2. the error envelope — the server's `{error, message, details?}` body is
 *     turned into a typed `ApiError` before it reaches callers;
 *  3. the 401 convergence — when a request dies with 401 we notify the
 *     registered listener (see hooks/use-session.ts), which clears the cache
 *     and sends the user to /login — but only if a session actually existed,
 *     so the /me bootstrap and the login page's own 401s are unaffected.
 *     The change-password endpoint is carved out: its 401 means "current
 *     password is wrong", not "session died" (see the interceptor below).
 */
import { client } from "./generated/client.gen"

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
    readonly details?: unknown,
  ) {
    super(message)
    this.name = "ApiError"
  }
}

// The server mounts everything under /api. Local Vite proxies `/api` onto
// the same origin so cookies stay first-party. A hosted split (platform
// on one host, API on another) must set VITE_API_BASE_URL to the API's
// `/api` origin at build time — otherwise the browser posts login at the
// static host and nothing happens.
client.setConfig({
  baseUrl: import.meta.env.VITE_API_BASE_URL || "/api",
  credentials: "include",
})

type SessionDeathListener = () => void
let sessionDeathListener: SessionDeathListener | null = null

export function onSessionDeath(listener: SessionDeathListener | null) {
  sessionDeathListener = listener
}

// Runs on every response, before the ok-check, so it sees 401s regardless of
// throwOnError. The change-password endpoint is carved out: its 401 means
// "current password is wrong", not "your session died" — the session is
// still live, and evicting the user over a typo would be worse than the
// typo itself.
client.interceptors.response.use((response) => {
  if (
    response.status === 401 &&
    !response.url.includes("/v1/auth/password")
  ) {
    sessionDeathListener?.()
  }
  return response
})

// The client throws the parsed error body; normalize it into an ApiError so
// callers can rely on a single shape.
client.interceptors.error.use((error, response) => {
  const envelope =
    error && typeof error === "object" && !Array.isArray(error)
      ? (error as { error?: string; message?: string; details?: unknown })
      : {}
  return new ApiError(
    response?.status ?? 0,
    envelope.error ?? "error",
    envelope.message ?? "Request failed",
    envelope.details,
  )
})

export { client }
