/// Vite's env typing: every `VITE_` var the platform reads. The values
/// live in `.env` (gitignored) — see `.env.example` for the roster.
interface ImportMetaEnv {
  readonly VITE_GOOGLE_MAPS_API_KEY?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
