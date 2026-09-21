/// Vite's env typing: every `VITE_` var the platform reads. The values
/// live in `.env` (gitignored) — see `.env.example` for the roster.
interface ImportMetaEnv {
    readonly VITE_GOOGLE_MAPS_API_KEY?: string
    /// Absolute API origin including `/api`, e.g.
    /// `https://tuma-utqv.onrender.com/api`. Absent = `/api` (Vite's
    /// same-origin proxy in local dev).
    readonly VITE_API_BASE_URL?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
