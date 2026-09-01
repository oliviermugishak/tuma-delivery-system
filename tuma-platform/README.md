# Tuma Platform — admin + merchant web dashboard

React 19 · TypeScript · Vite · Tailwind v4 · TanStack Router + Query ·
Orval-generated API client. Dark-only, built to the Phase-2 design
constitution (repo root: `DESIGN PRINCIPLES.md`, visual ground truth:
`../redesigns/dashboard_redesign.html`).

```bash
pnpm install
pnpm dev            # :3000, /api proxied to the local Rust server :8080
pnpm exec tsc --noEmit
pnpm build
```

After any server API change: `./run.sh openapi` (repo root) then
`pnpm generate:api`, and review the generated diff as part of the slice.
`src/api/generated/` is never hand-edited.

## Layout

```
src/
  api/            generated client (do not edit) + client.ts wrapper (auth,
                  error envelope, 401 convergence) + queries barrel
  components/
    ds/           the design-system vocabulary — every screen composes
                  from here; page-specific inventions are banned (P15)
    ui/           shadcn/base-ui primitives the ds layer sits on
    settings-screen.tsx   shared Settings (both wings, P16)
  features/
    admin/        Overview · Orders · Merchants(+detail, forms) · Riders ·
                  Customers · Disputes(+resolve) · Fees · Audit
    merchant/     Overview · Orders(board+history) · Menu(+product form) ·
                  Stores(+form, pin picker) · Store detail · Earnings · Reviews
    demo/seed.ts  demo rows for screens awaiting backend endpoints
    auth/         login
  lib/            format.ts (money/dates) · status.ts (dot+text vocabulary)
                  · session.ts · csv.ts
  routes/         file-based routes, two wings behind requireWing guards
```

## The law (short form)

One fact, one place · never render unknown data (no `—`, no `N/A`, no raw
IDs) · pages over modals (the only centered dialogs are P10 guard confirms)
· one accent button per view, carrying the amount when money moves · status
is dot + text · every table/chart has skeleton, empty, error, and content
states · no purple, no shadows, no gradients beyond skeleton shimmer and
≤10% chart fills · tokens live only in `src/styles.css`.

## Honesty notes

- Screens whose endpoints don't exist yet are marked `BACKEND GAP (Gn)` in
  code and catalogued with the needed contract in `BACKEND-GAPS.md`. They
  render demo rows from `src/features/demo/seed.ts` and their mutations
  stay client-side — nothing pretends to be server truth.
- The server owns truth: prices, statuses, totals are formatted, never
  recomputed; money is integer RWF.
