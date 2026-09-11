# DEPLOYMENT.md — how Tuma goes live

**Planning document, founder-ordered.** Covers every surface of the system —
Android, iOS, web platform, backend server, database, image storage — with
costs, speed, and scale in mind. Every cost is **≈ approximate** (published
pricing — verify at checkout); every capacity number is back-of-envelope
until real traffic replaces it.

---

## 0. Decisions locked (with their dates)

**The final stack, one line:** a single GCP `e2-medium` VM in Johannesburg
running Caddy + tuma-server + Postgres 18 (Docker compose) · images and DB
backups on R2 · platform on Bunny CDN (Johannesburg PoP) · a .com on
Spaceship · **≈ $35–45/mo total**.

| Date | Decision |
|---|---|
| 2026-09-04 | Android: **both channels in parallel** — direct APK (inner circle) + Play closed testing (§7) |
| 2026-09-04 | iOS: **Android-first**, full runbook in this document, ready on demand (§8) |
| 2026-09-04 | Domain: a **.com from Spaceship (≈ $8.88/yr)**; name TBD. This document uses the placeholder **`tuma.example`** everywhere — substitute the real domain |
| 2026-09-04 | Managed-DB candidate was **Neon** — superseded 2026-09-11 (§4.4) |
| 2026-09-11 | Hosting anchor, final: **an African region — everything near us**, even at the cost of PaaS ease. **GCP `africa-south1` (Johannesburg)**, one `e2-medium` VM; chosen over AWS (console friction), Vultr JNB (single zone), and every PaaS (no African region). §9 keeps the alternatives as a record |
| 2026-09-11 | Database, final: **Dockerized PostgreSQL 18, co-located with the app** (the speed-and-control play, §4) — Neon is EU-only, and the DB must always share the app's region (§4.4) |
| 2026-09-11 | Platform (web), final: **Bunny CDN + Bunny Storage** (Johannesburg PoP, ≈ $1–5/mo — paid as directed, not Cloudflare Pages); §6 keeps the fallbacks on record |

**The founding principle of this plan:** the founder's words — *"the problem
is not price; the problem is speed and delivery."* Every choice below
optimizes for the round-trip a Kigali phone actually experiences, and for
operational control (Docker, the same stack as dev, no managed-platform
black boxes), with the least configuration that still meets that bar.

---

## 1. The system in one picture

```
        Kigali rider phone ── GPS every 5s ──┐
        Kigali customer phone ── polls 5s ───┤    ≈ 35–50 ms RTT
                                             ▼
                     ┌───────────────────────────────────────────┐
                     │  GCP africa-south1 (Johannesburg)         │
                     │  ┌─────────────────────────────────────┐  │
                     │  │ VM (e2-medium, Docker compose)      │  │
                     │  │  caddy (TLS, HTTP/3) ──► tuma-server│  │
                     │  │                            │  <1 ms │  │
                     │  │                            ▼        │  │
                     │  │                    postgres:18      │  │
                     │  │  nginx ──► platform static (opt.)   │  │
                     │  └─────────────────────────────────────┘  │
                     └───────────────┬───────────────────────────┘
                                     │ nightly pg_dump -Fc (off-site vault)
                                     ▼
                       R2 (or B2/GCS) — backup vault + image origin
                       (immutable content-UUID keys, CDN-edge cached)

 platform.tuma.example ── Bunny CDN (Johannesburg PoP)
 images (media)         ── CDN edge ──► R2 origin (zero egress)
```

| Component | What it is | Where it runs | Recurring cost |
|---|---|---|---|
| `tuma-server` | Rust/Axum API, stateless | GCP VM, Docker | in the VM price |
| PostgreSQL 18 | All truth — orders, integer RWF, sessions | Docker, same VM (< 1 ms away) | in the VM price |
| `tuma-platform` | React/Vite static (admin + merchant) | Bunny CDN (Johannesburg PoP) | ≈ $1–5/mo |
| Image storage | `object_store` (S3-compatible) | R2 bucket — zero egress | ≈ $0 |
| DB backups | nightly `pg_dump -Fc` | → the same vault | ≈ $0 |
| Android app | Flutter APK/AAB | Play + direct APK | $25 one-time |
| iOS app | Same Flutter codebase | TestFlight/App Store (later) | $99/yr when launched |
| Maps/geocoding/routing | Google Maps Platform | Google SaaS | $0 under free tier + caps |
| Notifications (future slice) | FCM | Google, free | $0 |

**A hidden simplification worth naming:** V1 takes cash on delivery — no
payment processor, no PCI surface, no payouts. Money moves as integer RWF
rows in Postgres and banknotes in a rider's pocket.

---

## 2. Why Johannesburg — the speed ledger

Round-trip times from Kigali (approximate, measured class):

| Hosting location | RTT from Kigali | Verdict |
|---|---|---|
| **Johannesburg (GCP `africa-south1`, AWS `af-south-1`, Azure SAN, Vultr JNB)** | **≈ 35–60 ms** | **the anchor** |
| Nairobi/Kigali-local (Liquid Cloud, Africa Data Centres Kigali) | ≈ 2–15 ms | the extreme-later option (§9.2) |
| EU (Frankfurt-class) | ≈ 140–190 ms | the shape we are leaving |

What the African anchor buys, concretely, against the EU shape:

- **Every dynamic API call** — login OTP, browsing, checkout POST, every 5 s
  poll — drops ≈ 110–150 ms of transit. Polls matter less (5 s cycle);
  *interactive* moments matter more: checkout is a multi-statement
  transaction plus the client's sequential calls — the region move saves a
  felt ≈ 0.5–1 s across a placing-an-order flow.
- **App↔DB hop: sub-millisecond.** Co-located Dockerized Postgres beats any
  managed database on raw latency by design — there is no network between
  them, or a few hundred microseconds of Docker bridge at most.
- **First loads and images** ride CDN edge (Johannesburg-class PoPs) at
  ≈ 20–60 ms — origin location nearly irrelevant for immutable bytes (§5).
- **Mobile-network realities:** Caddy speaks HTTP/3 (QUIC), and the VM gets
  TCP BBR congestion control — both are cheap toggles that specifically
  help on lossy East African mobile paths.

The honest counterweight: African regions carry an **egress premium**
(≈ $0.12–0.14/GB vs ≈ $0.09 US). The architecture is already built to not
care: payloads are tiny (204-empty polls), images are zero-egress R2 served
from edge caches, and the platform bundle is static. The premium applies to
a few gigabytes a month of dynamic API bytes at pilot scale — cents.

---

## 3. Backend server — build and run

### 3.1 Packaging — Docker, because dev already is

One production `compose.yml` mirrors the dev stack exactly: **caddy
(reverse proxy, auto-TLS, HTTP/3) + tuma-server + postgres:18** (+ an
optional static-nginx for the platform, §6). Build is a multi-stage
`rust:1-slim` → `debian:bookworm-slim` image (rustls-only — no OpenSSL
system deps, image ≈ 50–80 MB). One small slice writes the Dockerfile +
compose; that is the entire "platform engineering" of this plan.

### 3.2 The VM

- **Shape:** `e2-medium` — 2 vCPU / 4 GB / 50 GB balanced disk,
  ≈ $30–40/mo in JNB. Capacity envelope (§10.1): ~2,000–5,000 concurrent
  tracking screens. Ubuntu LTS, Docker + compose plugin, nothing else.
- **Kernel/network:** enable TCP BBR; open 80/443 only; SSH keys only.
- **TLS:** Caddy obtains Let's Encrypt directly — no CDN required for
  correctness. If a CDN fronts any host later, switch that host to
  Full-strict with an origin cert.

### 3.3 Configuration and secrets (the exact inventory)

Layered YAML (`configuration/{base,production}.yml`) overridden by
`APP_*__*` env; `DATABASE_URL` wins over all database fields. Production
`.env` on the VM:

| Variable | Production value shape |
|---|---|
| `APP_ENV` | `production` |
| `TUMA_CORS_ORIGIN` | `https://platform.tuma.example` (exact origin; credentials allowed) |
| `APP_APPLICATION__HOST` | inside the compose network |
| `APP_APPLICATION__PORT` | `8080` |
| `APP_APPLICATION__COOKIE_SECURE` | `true` |
| `DATABASE_URL` | `postgres://tuma:…@postgres:5432/tuma` (compose-internal) |
| `APP_DATABASE__MAX_CONNECTIONS` | ≈ 40–60 (Postgres's own cap is 100 across *all* clients) |
| `APP_SECRET__JWT_SIGNING_KEY` | `openssl rand -base64 64` — generate once; back up with the DB (sessions die without it) |
| `APP_STORAGE__BACKEND` | `s3` (R2, §5) |
| `APP_STORAGE__PUBLIC_BASE_URL` | the R2 public/CDN URL |
| `APP_ROUTING__GEO_API_KEY` | the rotated Geocoding key (P13) |
| `APP_RATE_LIMIT__*` | on: 10 login + 10 OTP/min per IP (already built) |
| `APP_JOBS__PRUNE_ENABLED` | `true` — hourly prune of breadcrumbs + dead refresh tokens (no manual cleanup ever) |

Secrets live only in the VM's `.env` (gitignored). No secret in a repo, an
image layer, or a client bundle.

### 3.4 Migrations, deploys, rollback

- **Migrations:** `cargo sqlx migrate run` as the container start step,
  before serving. Forward-only: never edit a shipped migration.
- **Deploy:** `git pull && docker compose build && docker compose up -d` —
  seconds of swap, acceptable until the LB day (§10.2). The server is
  stateless (JWT cookies + DB-side refresh tokens): **no sticky sessions
  are ever needed**, which is what makes the multi-VM path boring.
- **Rollback:** previous image tag / `git revert`. Migrations are additive
  (house style), so the previous binary survives them.
- **Monitoring:** `/api/health` + UptimeRobot (free) → email; bunyan logs on
  stdout → docker json-file rotation.

---

## 4. Database — Dockerized, co-located, in control

### 4.1 The shape (and why it is the *fast* option, not the cheap one)

PostgreSQL 18 runs as a container in the same compose stack as the API.
App→DB is a Docker-network hop: **< 1 ms**, versus 2–150 ms for any
networked managed database. For a poll-probe workload — one indexed query
every 5 s per open screen — co-location is the single biggest latency win
available, and it is also maximal control: same engine, same version, same
migrations as dev; `docker compose` is the only ops interface.

Production discipline that makes Dockerized Postgres respectable:

- data on a **named volume** (containers die; data doesn't);
- `shared_buffers ≈ 25%` of RAM (≈ 1 GB on the 4 GB VM);
- healthcheck + `restart: unless-stopped`;
- migrations run on app start (single writer during deploy);
- **backups off-box, always** (§4.2) — the box is not the backup.

### 4.2 Backups (non-negotiable from day one)

- Nightly `pg_dump -Fc` → an off-site vault: **R2** (zero egress, 10 GB
  free — a dump vault is storage, not hosting), or Backblaze B2 / a GCS
  bucket if provider purity is preferred. Retention: 14 daily + 4 weekly
  via lifecycle rules.
- **Honest posture:** RPO ≤ 24 h, RTO ≈ 30 min (restore + verify). For a
  cash-on-delivery pilot this is proportionate; the upgrade trigger is in
  §10.2 (WAL-G/PITR or managed HA).
- **Quarterly restore drill:** dump → scratch container → `sqlx migrate
  run` no-ops → app boots against it. A backup never restored is a hope.
- The JWT signing key backs up alongside the dumps.

### 4.3 The managed path (later, and it stays in-region)

When HA matters (§10.2 triggers): **Cloud SQL Postgres in
`africa-south1`** (≈ $25–50/mo at small sizes) — automated backups, PITR,
zonal HA. The move is the same one-command pattern: dump → restore →
repoint `DATABASE_URL`. Vultr's managed Postgres (if JNB-located, verify)
is the lighter alternative on the budget path. The DB never multiplies
with the app — it is the one stateful piece.

### 4.4 The Neon chapter, recorded honestly

Neon was chosen on 2026-09-04 as the managed destination, then **superseded
on 2026-09-11** by the founder's "everything near us" directive — the two
are architecturally incompatible: Neon is EU-only, and a JNB app talking to
an EU DB would pay ≈ 150 ms on *every* query, including each poll probe and
every statement of the checkout transaction (§2's rule one level down:
**the app and DB must share a region, always**). Neon's genuine advantages
— zero-ops, PITR, free tier — are exactly what §4.3's Cloud SQL path
provides in-region, at pilot-scale prices that no longer matter to the
business. Nothing was ever deployed on Neon, so the reversal costs one
decision, not a migration. The exit lesson is worth keeping: the database
layer is vanilla Postgres with `pg_dump`/restore in and out — the DB home
is a reversible choice as long as the nightly vault exists.

---

## 5. Image storage — zero-egress origin, edge-served

The storage layer already has `local | memory | s3` behind one code path;
keys are content-UUIDs (never overwritten) and `Cache-Control: immutable`
is already set. **The origin's location barely matters for immutable bytes
— the edge does the serving.** That insight decides this section:

- **Origin: Cloudflare R2** (recommended). 10 GB + generous ops free,
  ≈ $0.015/GB-mo beyond, **zero egress fees** — structurally correct for
  the highest-volume bytes in the system, especially over Africa's
  expensive transit. Wiring is config-only (`backend: s3`,
  R2 endpoint, bucket, keys; `public_base_url` → the R2 public/CDN URL).
  A CDN in front (Bunny, §6, has a Johannesburg PoP) makes every image a
  ≈ 20–60 ms fetch for Kigali users.
- **Where the bucket physically lives (the honest answer):** R2 has no
  African site — the location hint is set to the closest region
  (Europe). This is by design invisible: images are immutable
  content-UUID keys, so the Johannesburg edge caches each image after
  its first fetch, and every later Kigali request is served from the
  edge. The European origin serves exactly once per image, ever.
  **Why it is free:** R2's free tier (10 GB stored, 1 M writes, 10 M
  reads per month) covers years of pilot imagery (≈ 300 MB projected),
  and — the structural part — Cloudflare charges **no data-transfer
  fees on R2 at all**, which is the line item that usually kills image
  budgets on AWS S3. The DB backup dumps ride the same free tier in a
  separate bucket.
- **Provider-purity alternative:** GCS in `africa-south1` — single vendor,
  in-region origin, but paid egress (≈ $0.12/GB) on every cache miss and a
  small code slice (the storage crate wires the S3 builder; GCS support is
  another arm of `object_store`). Not recommended while R2 is free of both.
- Uploads always enter through the API (validate → 5 MB cap → dimension
  guard → store): bad bytes never reach the bucket.
- Scale picture: 100 stores × 20 items × ≈ 150 KB ≈ 300 MB. Years of
  product photos fit in the free tier.

---

## 6. Web platform — paid, near, not Cloudflare Pages

The platform is a static Vite bundle (`pnpm build` → `dist/`); its host
needs to do one thing well: serve files fast. **Chosen: Bunny CDN +
Bunny Storage** — Johannesburg PoP, ≈ $1–5/mo total, dead-simple panel,
paid as directed. `platform.tuma.example` pulls from a Bunny storage
bucket fed by the deploy script (or CI); every Kigali fetch rides the
African edge.

Fallbacks, kept on record only:

- **Same-VM nginx:** the compose stack gains one static container; the
  platform ships with every API deploy, zero extra vendors, JNB origin.
  The choice if Bunny ever disappoints.
- **Vercel Pro / Netlify / Render Static:** best DX and preview deploys;
  no African edge — acceptable because the bundle is static and
  cacheable, but it is the only shape where "near us" is satisfied by
  cache rather than by geography.

**The auth rule that outlives the host choice:** the platform calls
`api.tuma.example` with `credentials: 'include'`. Session cookies stay
same-site because platform and API share **one registrable domain** —
this is a property of the *domain*, not the *provider*, so any host from
the list works. The server's `TUMA_CORS_ORIGIN` allows exactly the
platform origin, with credentials. Keep both under the domain bought in
§0 and this all just works forever.

The server deliberately does **not** serve the SPA (the static-dir code
was removed as dead in patch P20); the same-VM nginx fallback, if ever
used, is a separate, deliberate container.

---

## 7. Android — two channels in parallel

### 7.1 Signing first (the map-breaking gotcha)

1. Generate the **release upload keystore**; back it up in two physical
   places plus a password manager. A lost keystore is a lost app identity.
2. **The gotcha that costs days if missed:** the Google Maps Android key is
   restricted to package `com.tuma.tuma_app` + specific SHA-1s. A release
   keystore has a **different SHA-1** — add it to the Google Console
   restriction the same day the keystore is created, or every store build
   ships with blank maps. (Debug SHA-1 stays for dev.)

### 7.2 Channel A — direct APK (pilot, inner circle)

```
flutter build apk --release \
  --dart-define=TUMA_API_BASE_URL=https://api.tuma.example \
  --dart-define=MAPS_API_KEY=<android-key>
```

Distribute by link (Drive/Telegram/WhatsApp — normal in Kigali). Manual
reinstalls for updates; versionCode increments every build. Cleartext HTTP
stays debug-only. No fees, no review, instant iteration.

### 7.3 Channel B — Play Store closed testing (running toward production)

- Play Console: **$25 one-time**, identity verification.
- **The calendar rule:** new personal accounts must run a **closed test
  with 12 testers for 14 continuous days** before production. Recruit the
  inner circle + merchants (the direct-APK users double as testers). Start
  the 14-day clock as early as possible.
- Listing requirements: privacy policy URL (`platform.tuma.example/privacy`),
  data safety form (location for delivery; name/phone for accounts; **no
  financial data** — cash on delivery), content rating questionnaire.
- Production after the gate: staged rollout (10% → 50% → 100%), Play App
  Signing on.

---

## 8. iOS — the complete runbook (execute when the founder says go)

The Flutter code is already platform-ready; the blockers are procedural:

1. **Apple Developer Program: $99/yr.**
2. **A Mac, one way or another:** a used Mac mini (≈ $300–500 one-time) or
   **CodeMagic** cloud Macs (≈ $10–30/mo at light cadence). The dev machine
   is Linux — Xcode does not run on it.
3. **A new Google Maps SDK for iOS key**, restricted to bundle id
   `com.tuma.tuma_app`, same `MAPS_API_KEY` mechanism.
4. `flutter build ipa` → upload → **TestFlight** (internal same-day;
   external needs a light beta review) → App Store review (1–7 days;
   expect one rejection round on first submission — normal).
5. Store requirements: privacy nutrition labels (location, name, phone),
   location purpose strings (`NSLocationWhenInUseUsageDescription`),
   privacy policy URL. Apple sign-in **not** required (no third-party
   logins). IAP rules don't apply — cash on delivery.
6. **For the future background-delivery slice:** background location needs
   `NSLocationAlwaysAndWhenInUseUsageDescription`, the `location` background
   mode, and an App Review justification. Documented now so it never
   surprises a submission.

Realistic runway once go: **1–2 weeks**. Kigali's installed base is
≈ 85–90% Android, which is why this is second.

---

## 9. Hosting options — the full decision table

### 9.1 The African anchor (chosen)

| | **GCP `africa-south1` (default)** | **AWS `af-south-1`** | **Azure South Africa North** | **Vultr Johannesburg** |
|---|---|---|---|---|
| RTT to Kigali | ≈ 35–50 ms | ≈ 40–60 ms | ≈ 40–60 ms | ≈ 40–60 ms |
| VM (2 vCPU/4GB) ≈/mo | $30–40 | $30–40 | $30–45 | $24 |
| Managed Postgres in-region | Cloud SQL (later) | RDS (later) | Azure DB | Vultr managed (verify JNB) |
| Multi-AZ / real cloud | yes | yes (3 AZs) | yes | single zone |
| Load balancer for the 2–3 VM day | GCP LB (managed) | ELB/ALB | Azure LB | Vultr LB ≈ $10 |
| Config burden | moderate | **high (IAM/VPC)** | high | **low — plain VM** |
| Growth paths | Cloud Run (container scale), Cloud SQL, GKE if ever | the full AWS catalog | full Azure | mostly vertical |
| Verdict | **best speed/ease/scale blend** | enterprise gravity, solo-founder tax | same, less familiar | **the easy near box** |

Both GCP and AWS keep the "solo founder running Docker on a VM" workflow
identical to this plan; GCP's console and IAM are simply kinder, and Cloud
Run gives a managed-scaling exit later without a re-architecture. Vultr JNB
is the simplest and cheapest of the three but is single-zone — acceptable
for the pilot, noted honestly.

### 9.2 The extreme-later option: Kigali-local

Liquid Cloud / Africa Data Centres Kigali put the stack **2–15 ms** from
Kigali users — the absolute latency floor, real data residency. The costs
are equally real: quote-based pricing, VMware-era tooling, you are the SRE,
single-city risk (Rwanda rides undersea cables via Mombasa/Dar es Salaam —
a fiber event is a national event), and vertical-only scaling. This is the
move for a Tuma with a Kigali ops team, not a Tuma with a founder. It is
recorded so the path is known, not because now is its time.

### 9.3 What was rejected, and why

- **Render / Railway / Fly / any PaaS:** no African region — fails the
  anchor. (Render specifically: Oregon/Frankfurt/Ohio/Singapore/Virginia
  only.) They remain fine EU answers if the anchor ever flips back.
- **Neon:** EU-only; superseded (§4.4).
- **Kubernetes/orchestration:** 2–3 instances is a load balancer plus two
  VMs, not a control plane. Never at this scale.
- **EU VPS (Hetzner-class, ≈ $5):** the old Tier-1 default — kept as a
  footnote only: it loses to JNB on exactly the axis (speed) this plan now
  optimizes, and saves money the founder has said does not matter.

---

## 10. Scale plan — triggers, not guesses

### 10.1 Capacity envelope of the pilot VM (2 vCPU / 4 GB, app + PG)

- **Poll probes:** each ≈ 1 indexed query, answered from RAM. Postgres on
  2 vCPU serves thousands/sec; 1,000 customers with tracking open = 200
  probe rps → **< 10% of the box**.
- **Rider pushes:** 200 riders × 1 push/5 s = 40 rps, of which the
  ≥ 25 m/15 s noise rule writes ≤ 13 rps.
- **Real ceiling:** pool exhaustion and disk I/O before CPU — envelope
  **≈ 2,000–5,000 concurrent tracking screens**, years past pilothood.

### 10.2 The trigger table

| Signal (sustained) | Move |
|---|---|
| VM CPU > 60% for a week | Bigger VM (e2-standard-2, minutes of downtime) |
| Pool acquire timeouts / DB CPU > 40% | Tune pool, then **Cloud SQL `africa-south1`** (§4.3) |
| A second app VM exists | LB in front: GCP LB (managed) or Caddy-front-VM (cheap DIY at 2–3 nodes); add **pgbouncer** once >2 app VMs share the DB |
| 24 h RPO becomes unacceptable | WAL-G/PITR on the vault, or Cloud SQL |
| Poll load dominates CPU | WebSocket/SSE slice (blueprint next-steps) — the architecture already isolates reads |
| Kigali ops team exists, latency floor wanted | §9.2 local DC conversation |
| Image traffic > free R2 ops tier | Already solved — R2 scales at cents, $0 egress |

The property that keeps this plan boring: the server is stateless, the DB
is the only stateful piece, the image origin is config. Every move is
additive; nothing is ever a rewrite.

---

## 11. Cost summary

**One-time:** domain ≈ $8.88/yr (Spaceship .com) · Play Console $25 ·
keystore ritual $0 · (iOS later: $99/yr + Mac/CodeMagic).

**Monthly by stage (≈, USD, JNB anchor):**

| Line | Pilot | Growing (~1k orders/day) | Scaling (~10k/day, multi-VM) |
|---|---|---|---|
| VM + disk (GCP JNB) | $30–40 | $40–60 (e2-standard-2) | 2–3 VMs ≈ $80–140 |
| Database | in VM price | in VM price (or Cloud SQL $25–50) | Cloud SQL $40–80 |
| Image origin (R2) | $0 | $0–2 | $2–8 |
| Platform host (Bunny) | $1–5 | $1–5 | $5–10 |
| LB | — | — | $10–20 (GCP LB / Vultr LB) |
| Maps (free tier + caps) | $0 | $0 | $0–20 |
| Monitoring (UptimeRobot) | $0 | $0 | $0–7 |
| **Total** | **≈ $32–45/mo** | **≈ $45–95/mo** | **≈ $140–265/mo** |

The pilot is two restaurant dinners a month for a stack that is fast in
Kigali, controlled end-to-end, and Docker-identical to dev.

---

## 12. Security & reliability checklist (deploy day)

- [ ] HTTPS everywhere: Caddy auto-TLS (Let's Encrypt); HTTP/3 enabled
- [ ] TCP BBR on the VM kernel
- [ ] `APP_APPLICATION__COOKIE_SECURE=true`; auth cookies host-only on
      `api.tuma.example`
- [ ] `TUMA_CORS_ORIGIN` = exact platform origin — no wildcard, ever
- [ ] Rate limiting on the auth doors confirmed live (10/min per IP)
- [ ] Secrets only in the VM's `.env`; repo and image layers clean
- [ ] SSH: key-only, no password root, unattended security upgrades on
- [ ] Firewall: 80/443 open, everything else closed; Postgres not
      externally reachable (compose-internal network only)
- [ ] Nightly `pg_dump` → vault **and one verified restore** before
      calling it live
- [ ] UptimeRobot on `/api/health`; logs rotating; prune job enabled
- [ ] Maps keys: three keys, each restricted (Android package+SHA-1s, web
      referrer, server Geocoding/Directions), quota caps set, billing alert
      at $1

Known deliberate gaps (pilot-acceptable, each with its §10.2 trigger):
single VM (until the LB day), 24 h RPO, no WAF beyond the CDN's defaults.

---

## 13. Founder action checklist (ordered by lead time)

1. **Buy the .com on Spaceship** (≈ $8.88) → DNS wherever convenient
   (registrar's, GCP Cloud DNS, or Bunny DNS — DNS is not hosting; any of
   them works).
2. **Create the GCP project + billing** (card already proven with Google
   billing for Maps) → the VM in §3.2.
3. **Create the release keystore** + backup ritual → add its SHA-1 (and
   Play App Signing's, later) to the Google Console key restriction.
4. **Start the Play clock:** Console account ($25) → closed-testing track →
   recruit the 12 testers.
5. **Maps quota caps + billing alert** in Google Console (5 minutes).
6. **Privacy policy page** on the platform (Play requires the URL).
7. **Bunny account** → storage bucket + pull zone → `platform.tuma.example`
   (the deploy script feeds the bucket).
8. When the notifications slice is approved: Firebase project (free).
9. When iOS goes: Apple Developer ($99/yr) → Mac-or-CodeMagic choice (§8).

---

## 14. First-deploy-day runbook (bare VM → live)

1. Create the GCP project; boot `e2-medium` in `africa-south1` (Ubuntu
   LTS); SSH with keys; `apt` upgrades; install Docker + compose.
2. Enable BBR (`net.ipv4.tcp_congestion_control=bbr`); open 80/443.
3. Clone the repo; write the production `.env` from §3.3 (fresh
   `jwt_signing_key`; `cookie_secure=true`; `TUMA_CORS_ORIGIN` set).
4. `docker compose up -d` — postgres first (healthcheck), then the server
   (its entrypoint runs `sqlx migrate run`, then serves), then caddy —
   which obtains the Let's Encrypt cert for `api.tuma.example`.
5. `curl https://api.tuma.example/api/health` → 200.
6. Deploy the platform to its §6 host; point `platform.tuma.example`; log
   in; verify a store, an order, and an image URL render.
7. Create the R2 bucket + token; flip `APP_STORAGE__BACKEND=s3` +
   `public_base_url`; upload one product image; fetch it through the CDN.
8. Install the signed release APK on a real phone (real GPS, real maps —
   the §7.1 SHA-1 must be in the Console by now).
9. **The acceptance walk:** place a two-store order → merchant hands off →
   watch the gold route and the gliding rider dot → delivered → cash
   settles. The demo that proves the deploy.
10. Enable UptimeRobot; first `pg_dump` → vault → **restore into a scratch
    container and boot the app against it**.
11. Log the deploy (date, commit, image digest) in MEMORY.md — every deploy
    after this is one line: build, up -d, watch health.

---

## Appendix A — Stakeholder costing summary

### One-time setup costs

| Item | Cost (USD) |
|---|---|
| Web domain (.com, Spaceship) | ≈ $9 / year |
| Google Play developer account | $25 once |
| App signing key & certificates | $0 |
| **Total to launch** | **≈ $34** |

*(iOS, when launched: $99/yr Apple Developer + ≈ $300–500 one-time for a
build machine — or ≈ $10–30/mo cloud CI.)*

### Monthly running costs — by growth stage

| Line item | Pilot (launch) | Growing (~1,000 orders/day) | Scale (~10,000 orders/day) |
|---|---|---|---|
| Application servers — Google Cloud, Johannesburg | $30–40 | $40–60 | $80–140 (2–3 servers) |
| Database (PostgreSQL) | included | included *(managed: $25–50)* | $40–80 (managed) |
| Image & file storage — zero data-transfer fees | $0 | $0–2 | $2–8 |
| Web platform (admin + merchant console) — African CDN | $1–5 | $1–5 | $5–10 |
| Load balancing | — | — | $10–20 |
| Google Maps platform (free tier, hard-capped) | $0 | $0 | $0–20 |
| Push notifications (Firebase) | $0 | $0 | $0 |
| Uptime monitoring | $0 | $0 | $0–7 |
| **Total per month** | **≈ $32–45** | **≈ $45–95** | **≈ $140–265** |
| **Infrastructure cost per order** | **≈ $0.02–0.05** | **≈ $0.002–0.003** | **≈ $0.001** |

### The five points stakeholders should take away

1. **Unit economics improve with growth** — infrastructure falls from
   cents per order to a tenth of a cent at scale.
2. **Zero payment-processing fees in V1** — cash on delivery; no 2–3% +
   fixed processor cut per transaction; the ledger is ours.
3. **Everything hosted in Africa (Johannesburg)** — ≈ 40 ms from Kigali
   users, from a real multi-zone cloud.
4. **The biggest cost drivers are engineered to zero** — CDN-cached
   images from zero-egress storage, Maps inside a hard-capped free tier,
   free monitoring and notifications.
5. **Scaling is additive, never a rewrite** — one server grows into a
   load-balanced cluster by adding instances; one ops tool (Docker),
   identical to development; no specialized infrastructure hires.

*Footnote for the deck: planning figures at published 2025 prices — the
first month of real operation replaces ranges with measured invoices.*

---

*Costs and capacities above are honest approximations for planning, not
quotes. The first real deploy replaces the envelopes with measurements, and
this document gets the numbers written back into it.*
