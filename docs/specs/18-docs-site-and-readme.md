# 18 — Open-source documentation: site + README + CONTRIBUTING

> Design spec (brainstorming output). Feeds writing-plans → agent-brief.
> Goal: make Filature presentable as an open-source project — a styled
> single-page documentation site, a root README, a CONTRIBUTING guide, and an
> MIT LICENSE. All content in **English**.

## Problem / intent

Filature is going open source. It currently has internal docs (`docs/**`) written
for contributors, but nothing that presents the project to an outside audience:
no README, no public docs site, no contribution guide, no license. A newcomer
landing on the repo can't tell what it does, what it looks like, how to install
it, or how to help.

This slice produces the **public-facing** documentation layer, visually aligned
with the app itself.

## Deliverables

1. **`site/`** — a single-page documentation website (`index.html` + `style.css`
   + `script.js` + `img/`), served by **GitHub Pages** from the repo. No build
   step, no runtime CDN dependency.
2. **`README.md`** (repo root) — the GitHub landing page.
3. **`CONTRIBUTING.md`** (repo root) — how to build, run, test, and contribute.
4. **`LICENSE`** (repo root) — **MIT**, holder "ziggornif", year 2026.
5. **`.github/workflows/pages.yml`** — deploy `site/` to GitHub Pages on push to
   `main`.
6. **CI fix — multi-arch image** (`.github/workflows/ci.yml`): add
   `platforms: linux/amd64,linux/arm64` to the existing `docker` job's
   `build-push-action` step, so `ghcr.io/ziggornif/filature` runs on Raspberry Pi
   (arm64). Without this the site's RPi claim is false. (Verified feasible —
   see Installation.)

Optional (nice-to-have, not blocking): `CODE_OF_CONDUCT.md` (Contributor
Covenant) referenced from CONTRIBUTING.

## Non-goals

- No multi-page docs generator (mdBook/Zola). One hand-written page, like the
  reference project `ziggornif/gimme` (`docs/site/`).
- No JS framework, no build tooling, no npm in the shipped site.
- No API reference (Filature has no public API surface to document).
- Not documenting deferred/unbuilt features as if they exist — the roadmap
  section is explicitly forward-looking.

## Reference & aesthetic

**Structure** follows `ziggornif/gimme`'s `docs/site/` (sticky header with
logo + nav + GitHub button, left sidebar scrollspy, hero, feature-card grid,
callouts, steps, tables, footer; `script.js` handles mobile menu, scrollspy,
and — new here — the theme toggle).

**Skin** is Filature's own, NOT gimme's blue SaaS look. Reuse the app's design
tokens verbatim from `init_assets/design_handoff_filature/README.md`:

- **Palette**: warm neutral greys, 3 surface levels, slate accent `#5b5563`
  (light) / `#6b7686` (dark). Semantic `--ok`/`--warn`/`--danger` used only for
  status callouts, never decoration.
- **Full light + dark token sets** copied verbatim from the handoff (§Couleurs).
- **Typography**: IBM Plex Sans (UI) + IBM Plex Mono (all figures, code,
  version/port/env-var tokens). Load them the **same way the app does** — the
  Google Fonts `<link>` (`family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Sans:wght@400;500;600;700&display=swap`).
  The repo ships no woff2 files and there's no build step to fetch them, so
  self-hosting is out of scope; a `system-ui`/monospace fallback stack covers a
  blocked CDN. (This is the ONLY permitted external request.)
- **Radii**: cards 11–12px, controls 6–8px, status pills 20px. Grid gap 14px.
- Drop gimme's highlight.js entirely — shell snippets are short; style `<pre>`
  with plain mono. Zero third-party JS/CSS.

**Light + dark parity is the site's signature** (it is the app's). The site:
- defaults to the reader's OS `prefers-color-scheme`,
- exposes a persistent manual toggle (Auto/Light/Dark) in the header, stored in
  `localStorage`, applied as `data-theme` on `<html>`,
- and **swaps every screenshot** to match the active theme (see below).

## Site sections (single page, in order)

Sidebar nav links to each `section[id]`.

1. **Hero** — wordmark + tagline ("A self-hosted filament stock manager for your
   3D-printing workshop — track every spool, weight, value and printer from one
   calm dashboard."). A hero screenshot (Dashboard). Primary CTAs: **GitHub**
   (primary), **Install**. Small badges: Rust · Single binary · Self-hosted ·
   Light & dark. (No public "Live demo" link — the instance runs on a private
   home VM and is not exposed.)
2. **Features** — a card grid, with screenshots interleaved. One card each:
   - **Dashboard** — stock value (€), remaining weight, spool counts, low-stock
     alerts, split by material, soon-empty list.
   - **Spools** — dense filterable/sortable list (table + card views), inline
     remaining-weight edit, 2-screen add/edit wizard (net weight, no tare),
     status lifecycle (Sealed/Open/Empty/Archived).
   - **Spool detail** — remaining in g + length (m) + %, weight gauge, full
     identity/purchase info, notes.
   - **Materials referential** — editable table: density, drying params,
     humidity sensitivity, default nozzle/bed temps. Single source of truth for
     length and (future) humidity thresholds.
   - **Locations** — storage places for spools.
   - **Printers** — Bambu multi-AMS topology, multiple heads, per-slot spool
     loading with filament colour pastilles, feed mode.
   - **Settings** — tabs, low-stock alert threshold, locale, theme.
   - **Export / Import** — full instance backup/restore as a portable file.
   - **International & themed** — English + French UI, light + dark at parity.
3. **Screenshots** — a small gallery of the key screens (theme-synced, English).
4. **Installation** — see below.
5. **Contributing** — short blurb + link to `CONTRIBUTING.md` and the repo.
6. **Footer** — project name, MIT, links (GitHub, demo, license), "Zig Factory ·
   self-hosted".

> **No roadmap section on the site.** The forward-looking items live only in the
> README (see below). The site documents what exists today.

## Screenshots

- **Source**: a running Filature instance seeded with demo data, logged in
  through its login gate. (No public URL/credentials recorded here — the
  instance is on a private home VM.)
- **Capture tool**: Playwright (already at v1.27 via `npx`; run
  `npx playwright install chromium` first). A capture script under
  `tools/screenshots/` (throwaway/dev tooling, kept for re-runs) that: logs in,
  sets UI locale to **English**, then for **each** target screen captures it in
  **both light and dark** theme.
- **Screens (captured — DONE)**: 10 screens × 2 themes = **20 PNGs already in
  `site/img/`**. Exact filenames (each has a `-light` and `-dark` variant):
  `dashboard`, `spools-cards`, `spools-table`, `spool-detail`, `spool-new`
  (wizard condition screen), `spool-new-details` (wizard details screen),
  `materials`, `printers`, `settings-locations`, `settings-backup`.
- **Output**: `site/img/<screen>-{light,dark}.png`, viewport 1360×900,
  deviceScaleFactor 2 (retina). Sizes 100–400 KB (avg ~210 KB, ~4.2 MB total).
  Optional later: run `pngquant` to shrink — not installed here, not blocking.
- **Capture note (for re-runs)**: Playwright ≥ 1.61 required — the sidebar nav
  uses the modern `::details-content` pseudo-element, which the old bundled
  Chromium (Playwright 1.27) does not support, so the nav renders blank. The
  capture script also force-sets `<details class="sidebar-menu" open>` before
  each shot as a belt-and-suspenders. Log in through the gate; set cookies
  `lang=en` + `theme=light|dark`; use `waitUntil: 'domcontentloaded'`
  (`networkidle` hangs on this app).
- **Theme-synced display**: each `<img>` carries both sources; `script.js` sets
  the visible one from the active `data-theme` and updates on toggle. Provide
  descriptive `alt` text per image.
- **Privacy**: the demo data is synthetic (demo instance). Confirm no personal
  data is visible before committing PNGs.

## Installation section (content)

- **What it is**: a single self-contained Rust binary (templates, static assets,
  i18n catalogs, migrations all embedded) + a PostgreSQL database. HTTPS is
  terminated by a reverse proxy you run separately.
- **Minimum specs — VERIFIED (2026-07-17):**
  - **Release binary: 12 MB** (`filature`, arm64 build). Self-contained
    (templates, static assets, i18n, migrations all embedded).
  - **Runtime Docker image: ~118 MB** (`debian:bookworm-slim` + binary +
    ca-certificates), arm64.
  - **RAM**: the Rust app idles small (well under ~50 MB RSS); **PostgreSQL is
    the larger consumer**. Realistic baseline: RPi 4 with **4 GB comfortable,
    2 GB workable** (Postgres + one app + OS). State the sizes as measured; frame
    RAM as guidance, not a hard floor.
  - **Disk**: ~120 MB image + Postgres data volume (grows with your stock; a
    personal instance is tens of MB).
- **Raspberry Pi 4 (arm64) — VERIFIED FEASIBLE, but needs a CI fix (deliverable
  of this slice):**
  - Proven locally: `docker buildx build --platform linux/arm64` compiles the
    workspace cleanly (~30 s on an arm64 host) — **all dependencies are pure
    Rust** (sqlx postgres has no libpq, argon2 pure Rust, no OpenSSL), so arm64
    has no native-lib blockers.
  - **BUT** the current `docker` job in `.github/workflows/ci.yml` (build+push to
    `ghcr.io/ziggornif/filature`) has **no `platforms:` field**, so it publishes
    an **amd64-only** image. `ghcr.io/ziggornif/filature:latest` will **not run
    on a Pi today.**
  - **Fix (in scope):** add `platforms: linux/amd64,linux/arm64` to the
    `docker/build-push-action@v6` step. On GitHub's amd64 runner the arm64 half
    builds under QEMU (a few minutes — acceptable; deps are pure Rust). After
    that, an RPi just runs `docker compose -f docker-compose.prod.yml up -d
    --pull always` and Docker pulls the arm64 variant automatically. Only then
    does the site say "runs on a Raspberry Pi 4 or newer".
- **Install steps** — two paths:
  - **Recommended (pull the published image):** the CI publishes
    `ghcr.io/ziggornif/filature:latest` (multi-arch after the fix above). Use a
    compose file that references the image (like `docker-compose.prod.yml`),
    `cp .env.example .env`, set `POSTGRES_PASSWORD`, hash a login with
    `docker compose run --rm app hash-password '<pw>'`, then
    `docker compose up -d --pull always`. This is the RPi path.
  - **From source:** clone, `cp .env.example .env`, set `POSTGRES_PASSWORD`,
    `docker compose up -d --build`.
  - Verify with `docker compose ps` / `curl`, then point a reverse proxy at the
    published port. (Note: the app has a login gate via `FILATURE_AUTH__*` —
    see ADR-0005 / the prod compose; on the bare `docker-compose.yml` dev setup
    there is no auth, so keep it private — see security note.)
- **Config table**: `POSTGRES_*`, `APP_PORT`, `APP_BIND`,
  `FILATURE_DEFAULT_LOCALE`, and the `FILATURE_<SECTION>__<KEY>` override rule.
- **Security callout** — **corrected from the stale `docs/deploy.md`.** Filature
  now has a **mandatory single-credential login gate**: the app refuses to boot
  without `FILATURE_AUTH__USERNAME` + `FILATURE_AUTH__PASSWORD_HASH` (argon2 PHC
  string from `filature hash-password`), and default-denies every path without a
  valid session cookie (see `crates/app/src/web/auth.rs`, ADR-0005). So it is
  NOT "wide open". Caveats to state honestly:
  - It's a **single shared operator credential**, not multi-user/roles.
  - **No TLS itself** — terminate HTTPS at your reverse proxy; keep the app port
    private (bind to a private interface / firewall to the proxy).
  - **Do not repeat deploy.md's "no built-in authentication" line** — it predates
    the auth gate. (Flag: `docs/deploy.md` should be corrected too, but that's a
    separate internal-docs fix, out of scope here.)
- **Backup/restore**: `pg_dump` / `psql` one-liners from `docs/deploy.md`, plus
  the in-app Export/Import.

## Roadmap (README only)

Lives in `README.md`, **not** on the site. Framed as "where Filature is going",
clearly not-yet-built. Items (in the order we discussed):

1. **Humidity monitoring** — the original differentiator. SHT31 sensors per
   drybox over MQTT, per-material humidity thresholds, a "dry before you print"
   alert for sensitive filament (nylon/PA-CF/PC). Already designed in the
   handoff; deferred until physical sensors exist.
2. **Per-print consumption & cost** — track filament used per print job and
   derive real €/g material cost per part (Zig Factory quoting).
3. **Spool entry via OCR** — scan a filament label to auto-fill the add form
   (brand, material, colour, weight) instead of typing it.
4. **More printer integrations** — beyond Bambu AMS: other printers, Spoolman
   import.
5. **Printers view UX** — refine the printers / AMS screen: clearer slot layout,
   faster spool loading, better at-a-glance state.
6. **Live printer-API connection** *(exploratory — "to be confirmed")* — connect
   to printer APIs (Bambu, Prusa, Klipper) to pull live information about running
   prints and loaded spools directly, instead of manual entry. Mark as an
   under-investigation idea, not a commitment.

## CONTRIBUTING.md (content)

- Project one-liner + link to the docs site.
- **Prerequisites**: Rust (edition 2024, ≥ 1.85 — image pins 1.97), Docker +
  Compose (for Postgres / the DB used by tests), and the `git submodule
  update --init` step for the craft-harness submodule.
- **Run locally**: `docker compose up` (or bring your own Postgres +
  `FILATURE_DATABASE__URL`), how the binary embeds assets (note: editing a
  static asset needs a rebuild — see the repo's local-run notes).
- **Tests**: `cargo test`; note the sqlx offline cache (`SQLX_OFFLINE=true`,
  `.sqlx/`) and, if relevant, the testcontainers Postgres usage +
  `tools/test.sh` reaper.
- **Architecture**: hexagonal + vertical slices; point at `docs/architecture.md`,
  `docs/adr/`, `docs/glossary.md`. i18n rule: no hardcoded UI strings
  (ADR-0001).
- **Workflow**: conventional commits (the repo history uses
  `feat(scope): …` / `fix(scope): …`), PRs against `main`, keep `.sqlx` and i18n
  catalogs in sync, CI must pass.
- **Scope note**: the internal `docs/specs/**` briefs and the craft harness are
  how slices are built — link them so contributors understand the process.
- Pointer to `CODE_OF_CONDUCT.md` if we add one.

## README.md (content)

Condensed GitHub landing (not a duplicate of the site):

- Title + one-line description + badges (license MIT, built with Rust).
- One hero screenshot (Dashboard) — reuse a `site/img/` PNG.
- **What is Filature** — 2–3 sentences.
- **Features** — tight bullet list (mirrors the site's feature cards).
- **Quick start** — the docker-compose steps (short version) + link to the full
  Installation section on the site.
- **Security** — short + accurate: mandatory single-credential login gate
  (argon2), TLS at the reverse proxy, keep the port private. NOT "no auth".
- **Roadmap** — one-line bullets (the 6 items above); README is the only place
  the roadmap lives.
- **Contributing** — link to `CONTRIBUTING.md`.
- **License** — MIT.

## GitHub Pages deploy

- `.github/workflows/pages.yml`: on push to `main` touching `site/**`, upload
  `site/` as a Pages artifact and deploy. Standard `actions/upload-pages-artifact`
  + `actions/deploy-pages`, `pages: write` / `id-token: write` permissions.
- Result URL: `https://ziggornif.github.io/filature/` (relative asset paths so it
  works under the `/filature/` sub-path).
- Add a `site/.nojekyll` so paths starting with `_`/`fonts` are served as-is.

## Acceptance

- Site renders correctly in light AND dark, screenshots swap with the toggle,
  **no external requests except the IBM Plex Google Fonts link** (no JS libs, no
  other CDN — drop gimme's highlight.js), responsive down to mobile
  (sidebar → menu), passes a basic a11y pass (alt text, focus states, skip
  link).
- README, CONTRIBUTING, LICENSE present at repo root; links between them and the
  site resolve.
- Installation section states **verified** minimum specs; the Raspberry Pi claim
  is backed by a working arm64 path (multi-arch image or documented on-Pi build),
  not an untested assertion.
- Pages workflow deploys the site successfully.

## Division of work

- **Claude Code (orchestrator)**: screenshot capture (browser + live-instance
  creds + locale/theme switching), and the RPi/arm64 verification.
- **Codex (implementation)**: the site HTML/CSS/JS, README, CONTRIBUTING,
  LICENSE, Pages workflow — per the repo's Codex-delegation convention. The
  captured PNGs and verified spec numbers are inputs handed to it.

## Open items

- ~~Exact minimum-spec numbers~~ — **RESOLVED**: binary 12 MB, image 118 MB
  (arm64, measured 2026-07-17).
- ~~arm64 strategy~~ — **RESOLVED**: multi-arch image via a one-line CI fix
  (`platforms: linux/amd64,linux/arm64`); build proven to work.
- Whether to add `CODE_OF_CONDUCT.md` now or later — author's call.
- `docs/deploy.md` still says "no built-in authentication" (stale) — correct it
  in a separate internal-docs pass.
