# Tuma — Build & Device-Testing Guide

**For agents and developers.** Everything you need to build, run, and test the
Tuma app on a real phone — including the parts that are NOT in the code: the
secrets' flow, the machine's quirks, and the traps that cost us hours. Read
`AGENTS.md` first (the working contract); this guide is the "how", it never
overrides the "what".

---

## 1. Repository map

| Path | What |
|---|---|
| `tuma-server/` | Rust backend: Axum + SQLx + PostgreSQL. `src/` is API-wiring ONLY — all domain logic lives in workspace crates (`accounts`, `commerce`, `marketplace`, `storage`, `routing`, `app-config`). |
| `tuma-app/` | Flutter customer + rider app (the APK). |
| `tuma-platform/` | Web admin + merchant wings (React, Vite, pnpm). Orval-generated client in `src/api/generated/` — never hand-edit. |
| `tuma-docs/` | Source of truth. `Tuma_V1_Brief.md` wins conflicts; the blueprint is long-term reference only. |
| `MEMORY.md` | Living project memory — read it, update it after durable changes. |
| `run.sh` | Day-to-day commands: `./run.sh api`, `./run.sh mobile`, `./run.sh openapi`, `./run.sh seed`. |

---

## 2. One-time machine setup (as built on the founder's Arch box)

- **Flutter** 3.47+ at `/opt/flutter` (on PATH). `flutter doctor` must be green for Android toolchain.
- **Android SDK lives OUTSIDE the project** at `~/Android/Sdk` (cmdline-tools, `platform-tools`, `platforms;android-36`, `build-tools;36.0.0`). `ANDROID_HOME` + PATH are set in `~/.bashrc` and `~/.config/fish/config.fish`.
- **JDK**: Temurin 25 via sdkman — works, do NOT install a second JDK.
- **There is NO system Gradle — on purpose.** Flutter drives the project's own Gradle wrapper (9.3.1). Never call `gradlew` directly for APK builds; `flutter build apk` runs `assembleDebug` under the hood. (You MAY use `cd android && ./gradlew processDebugMainManifest` to inspect manifest merging.)
- **Dev Postgres**: docker container `tuma-postgres-dev` (postgres/password123, db `tuma`). `DATABASE_URL` comes from sourcing `tuma-server/init_db.fish`. Migrations: `cargo sqlx migrate run`. After any query change: `cargo sqlx prepare --workspace` (the `.sqlx/` offline cache is committed — CI builds against it).
- **Emulators**: none in play. Desktop (Linux) runs the app with honest map placeholders; **real verification happens on the founder's Android phone** (see §7).

---

## 3. The secrets map — where every key lives and flows

| Key | Lives in | Flows to | Restricted to |
|---|---|---|---|
| `MAPS_API_KEY` | `tuma-app/.env` (gitignored) | `run.sh` sources it → Gradle reads `System.getenv` at **configuration time** → `manifestPlaceholders` → `AndroidManifest.xml` meta-data `com.google.android.geo.API_KEY` → the native Maps SDK | Google Cloud key: Android-app restriction = package `com.tuma.tuma_app` + the **debug keystore SHA-1**, API restriction = **Maps SDK for Android** |
| `TUMA_API_BASE_URL` | **NOT a file — a `--dart-define` at build time** | Baked into the binary. Nothing reads env at runtime. | n/a |
| `APP_ROUTING__BACKEND` / `APP_ROUTING__API_KEY` | `tuma-server/.env` (gitignored) | dotenvy → `RoutingConfig` → the `routing` crate's Google Directions client (server-side only; the key NEVER reaches a client) | Google Cloud key: Directions API enabled |
| `APP_ROUTING__GEO_API_KEY` | `tuma-server/.env` | `GoogleKeys.geocoding` → `GET /v1/geo/search` + `/v1/geo/reverse` proxies | Geocoding API enabled. **Unset = the address screen's search degrades honestly to map-drag + typing.** |
| `APP_STORAGE__PUBLIC_BASE_URL` | `tuma-server/.env` or env var | Composed into every image URL as `{base}/{key}` | — |

**The debug keystore never changes.** `~/.android/debug.keystore` is created
once and signs every debug build. Its SHA-1 (extract with
`keytool -list -v -keystore ~/.android/debug.keystore -alias androiddebugkey
-storepass android -keypass android | grep SHA-1`) is registered in the Google
Cloud console ONCE and stays valid forever. Only deleting that file changes it.

---

## 4. Building the app

### Desktop dev build (maps show honest placeholders)
```bash
cd tuma-app && flutter build linux --debug   # or: ./run.sh mobile from repo root
```

### The phone APK — the full correct command
```bash
cd tuma-app
set -a && source .env && set +a                      # MAPS_API_KEY into the env
flutter build apk --debug \
  --dart-define=TUMA_API_BASE_URL=http://<PC-LAN-IP>:8080
```
Output: `build/app/outputs/flutter-apk/app-debug.apk`.

Warm builds ≈ 40–60 s. First-ever build downloads the Gradle toolchain (~25 min).

### ⚠️ The twists (each of these bit us once)

1. **A bare `flutter build apk` / `flutter run` SILENTLY DROPS the maps key.** `build.gradle.kts` does `System.getenv("MAPS_API_KEY") ?: ""` — empty string, no failure, blank maps. Always source `tuma-app/.env` first, or use `./run.sh mobile` (it sources it).
2. **Gradle daemons cache the environment.** `System.getenv` is evaluated inside a long-lived daemon process. If you change `.env`, the old daemon keeps the old value. After any key change: `cd android && ./gradlew --stop`, then rebuild.
3. **The LAN IP is DHCP — it MOVES.** Check `ip -4 addr show` for the current
   `192.168.1.x` before EVERY APK build and before starting the API. It
   shifted `.100 → .101` mid-project and invalidated a built APK. There is no
   retry: the URL is baked into the binary.
4. **Android 9+ blocks plain HTTP.** The LAN API has no TLS, so the DEBUG
   manifest carries `android:usesCleartextTraffic="true"`
   (`android/app/src/debug/AndroidManifest.xml`). Without it every request from
   the phone fails. Release keeps Android's default and ships HTTPS.
5. **INTERNET permission lives in the MAIN manifest now.** It used to exist only
   in the debug/profile manifests (Flutter template dev permissions) — a release
   APK would have shipped with no network at all. Do not remove it.
6. **Maps only truly verify on a phone.** `isMobilePlatform` (android/iOS) gates
   the real `GoogleMap` widget; desktop gets a data placeholder by design. A
   green Linux build proves nothing about tiles. Blank tiles with a correct key
   in the manifest = the Google Cloud console restriction is wrong (package,
   SHA-1, or the SDK isn't enabled on the key). Propagation takes ~5 min.
7. **`APP_STORAGE__PUBLIC_BASE_URL` must INCLUDE `/api/v1/files`** — the server
   composes image URLs as `{base}/{key}`. `…:8080` alone produces broken image
   URLs. Empty value = relative `/api/v1/files/{key}` (fine on desktop, useless
   on a phone).
8. **Device testing is APK-only.** The founder's phone is Android 10 (wireless
   adb pairing is Android 11+) with a broken USB port. No `adb`, no hot reload
   on the phone — desktop carries UI iteration; the phone gets full builds.

---

## 5. Running the stack

```bash
# server (binds LAN for phone tests)
cd tuma-server && docker compose up -d          # postgres
source init_db.fish
cargo sqlx migrate run
APP_APPLICATION__HOST=0.0.0.0 \
APP_STORAGE__PUBLIC_BASE_URL=http://<PC-LAN-IP>:8080/api/v1/files \
  ./run.sh api                                   # from repo root; :8080

# platform (admin/merchant web)
./run.sh platform                               # :3000, proxies /api → :8080

# seed the Kigali pilot (idempotent — skips if merchants exist)
./run.sh seed
```

Handy logins (dev only): platform admin `admin@tuma.rw / Admin12345`; merchant
owners `simba@tuma.rw`, `kfc@tuma.rw`, `java@tuma.rw` — all `Password123`.
Customer/rider sign-in is phone + OTP; **the dev OTP is always `123456`**.
Riders are created in the admin wing (Riders page) and sign into the app with
their phone → the router lands them in rider mode.

Health: `curl http://<IP>:8080/api/health`. OpenAPI:
`./run.sh openapi` writes `tuma-platform/openapi.json`.

---

## 6. The phone-testing loop (two devices, one LAN)

1. **Firewall** (founder's sudo):
   ```bash
   sudo ufw allow from 192.168.1.0/24 to any port 8080 proto tcp comment 'tuma dev api'
   sudo ufw allow from 192.168.1.0/24 to any port 8081 proto tcp comment 'tuma apk download'
   ```
2. **Start the API bound to LAN** (command in §5).
3. **Stage + serve the APK:**
   ```bash
   mkdir -p ~/tuma-apk
   cp tuma-app/build/app/outputs/flutter-apk/app-debug.apk ~/tuma-apk/tuma-debug.apk
   cd ~/tuma-apk && nohup python3 -m http.server 8081 --bind 0.0.0.0 >/tmp/tuma-apk-server.log 2>&1 &
   ```
   Verify: `curl -sI http://<PC-LAN-IP>:8081/tuma-debug.apk` → `200 OK`.
4. **On the phone** (same Wi-Fi): browser → `http://<PC-LAN-IP>:8081/tuma-debug.apk`
   → download (~217 MB debug APK) → allow "Install unknown apps" for the
   browser (one-time) → install → open Tuma → grant location.
5. **Iteration loop:** rebuild → re-copy to `~/tuma-apk/` → re-download the SAME
   link. UI iteration happens on desktop; the phone gets real builds only.

---

## 7. Testing discipline (founder rules — breaking these is not optional)

- **NEVER run the full test suite.** `cargo test --workspace` and whole-file
  `flutter test` runs have crashed the founder's machine. Run ONLY your slice's
  tests: `cargo test --test <area>` / `flutter test --plain-name "<name>"`.
  The founder runs the global suite himself.
- **Pin `--concurrency=1` on every flutter test command.** A default-concurrency
  run (Chrome + Gradle daemons + test isolates on a 7.5 GiB machine) OOMed the
  whole system once. One test at a time, batch by name.
- Widget tests pin `TargetPlatform.linux` via the `testDesktop` wrapper
  (`test/test_desktop.dart`) because a bare `GoogleMap` in tests crashes
  (`RenderAndroidView` infinite-size), and geolocator platform channels HANG
  (not fail) in tests — screens take injectable seams
  (`acquireLocationProvider`) that tests override with stubs.
- Poll-based tests use bounded `pump()`s, never `pumpAndSettle` — timers never
  settle.
- Server suites are serial (`#[sqlx::test]` contention). Bars after any change:
  `cargo fmt && cargo clippy --workspace --all-targets` (0 warnings),
  `cargo sqlx prepare --workspace` (with `DATABASE_URL` set), `flutter analyze`
  clean.

---

## 8. Conventions the code enforces (and reviewers enforce harder)

- **Money is integer RWF** — never floats. The server owns every number; clients render, never compute.
- **Six order statuses**: `placed, accepted, preparing, picked_up, delivered, cancelled`. `picked_up` is entered ONLY by the merchant's handoff-by-rider-number endpoint; the rider's Delivered settles the cash allocation atomically.
- **Real tracking, never simulated**: GPS pushed by the rider's phone (≥25 m / ≥15 s breadcrumbs), road route cached at handoff (Directions key on the server), customer polls with `since` → 204-when-unchanged. `rider_distance_m` is server-computed (haversine to destination).
- **The Flutter API client is hand-written** (`lib/core/api/`) — typed per slice against the OpenAPI contract, never code-generated (the platform's IS generated).
- **Design tokens only in `lib/core/theme/`** — no raw hex anywhere else. Dark navy/gold identity, Inter everywhere.
- **New dependencies need a slice that names them.** Server house style mirrors `~/Work/projects/kanombe-sda` (see AGENTS.md).
- **Every server endpoint lands as one reviewed slice**: migration → domain crate → handler → registration → OpenAPI → integration test.
- **Founder-review-required** (never autonomous): payments/money, order state, migrations, auth, docs, new deps, deletions.

---

## 9. Troubleshooting table

| Symptom | Cause | Fix |
|---|---|---|
| Map area blank but pins/overlays visible | Key restriction mismatch (package/SHA-1/SDK) or key dropped at build | Verify console; check merged manifest (`build/app/intermediates/merged_manifest/debug/processDebugMainManifest/AndroidManifest.xml`); rebuild with `.env` sourced; `./gradlew --stop` first |
| Every request fails on the phone, works on desktop | Cleartext HTTP blocked, or baked IP stale | Debug manifest `usesCleartextTraffic` (present); re-check LAN IP; rebuild with current IP |
| Images broken on phone, fine on desktop | `APP_STORAGE__PUBLIC_BASE_URL` missing `/api/v1/files` or stale IP | Restart API with full base including `/api/v1/files` |
| `Failed to load … smoke_test.dart` at random | Machine OOM from parallel test isolates | `--concurrency=1`, test by name, close Chrome tabs |
| "Could not reach the server" when tapping rider Start | OLD bug — GPS exceptions were mislabeled | Fixed; if you reintroduce a generic catch, map `PermissionDeniedException` / `LocationServiceDisabledException` / `TimeoutException` first |
| Red "overflowed by N pixels" stripes on a small phone | Unshrinkable Text inside a bare Row | `Expanded`/`Flexible` + `maxLines`/ellipsis on the Text (see `StatusRow`) |
| OTP screen appears for a rider | Rider profile missing for that phone | Create the rider in the admin wing; riders skip customer creation by design |
| `since` query param wrong / no 204s | RFC-3339 `+` must be `%2B`-encoded in query strings | Use the api client's encoder (already handles it) |

---

## 10. Release checklist (when "shipping" becomes real)

- [ ] Release keystore + its SHA-1 registered on a NEW Google Cloud Android key.
- [ ] `flutter build apk --release --dart-define=TUMA_API_BASE_URL=https://api…` (HTTPS).
- [ ] Server: production yml + real secrets as host env vars (never `.env` in prod), `APP_STORAGE__BACKEND=S3`/R2 with CDN public URL.
- [ ] Cleartext flag is debug-only (verify it is NOT in the merged release manifest).
- [ ] INTERNET permission present in release manifest (it is now, in main).
- [ ] `sqlite`/dev conveniences (fixed OTP code, paste-coordinates field) verified off in production config.

---

*Maintained by the founder + agents. When you learn a new twist, add it here
and to `MEMORY.md` — the next agent inherits what you write.*
