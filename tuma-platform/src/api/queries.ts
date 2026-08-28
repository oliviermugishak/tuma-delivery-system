// Hand-written barrel over the generated TanStack Query options/mutations.
// Importing ./client first guarantees the generated client is configured
// (baseUrl, credentials, interceptors) before any query key is computed —
// query keys embed the client's baseUrl, so ordering matters.
import "./client"

export * from "./generated/@tanstack/react-query.gen"
