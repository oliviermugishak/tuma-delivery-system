import { defineConfig } from "@hey-api/openapi-ts"

// The API client is generated from the server's OpenAPI spec — never
// hand-edit src/api/generated. After any server API change:
//   ./run.sh openapi && pnpm generate:api
export default defineConfig({
  input: "./openapi.json",
  output: "./src/api/generated",
  plugins: [
    "@hey-api/typescript",
    "@hey-api/client-fetch",
    "@hey-api/sdk",
    "@tanstack/react-query",
  ],
})
