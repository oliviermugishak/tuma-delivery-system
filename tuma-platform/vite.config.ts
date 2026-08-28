import { defineConfig } from 'vite'
import { devtools } from '@tanstack/devtools-vite'

import viteReact from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

const config = defineConfig({
  resolve: { tsconfigPaths: true },
  plugins: [devtools(), tailwindcss(), viteReact()],
  server: {
    // The Rust server owns truth at :8080. Proxying /api keeps the browser
    // same-origin so the httpOnly session cookies flow and no CORS preflight
    // is needed in dev. The Origin header (http://localhost:3000) passes
    // through untouched, matching the server's TUMA_CORS_ORIGIN allowlist.
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8080',
      },
    },
  },
})

export default config
