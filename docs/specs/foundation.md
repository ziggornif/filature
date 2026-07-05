## Agent Brief

_AI-generated brief. Origin: orchestrator harness, phase 4._

**Category:** feature
**Summary:** Stand up the single-binary app substrate — boots (config → SQLite/WAL → embedded migrations), embeds all assets, and serves the app-shell in light/dark themes and en/fr locales — so domain slices have something to plug into.

**Slice / context:** Foundation (pre-slice substrate). Nothing exists yet: empty repo with only planning docs, CI, and the two architectural sensor scripts. This unit creates the Cargo workspace and the app shell; it introduces **no domain entity** (Material, Spool, Location come in their own slices). It is the prerequisite of every other slice.

**Desired behavior:**
- The build is a **two-crate workspace** ([ADR-0002](../adr/0002-crate-structure.md)): a pure `domain` crate (framework-free) and an `app` crate holding all adapters + the single binary. The domain-purity sensor (`tools/check-domain-purity.sh`) passes — `domain` depends only on the allowlist.
- Starting the binary: loads configuration from a TOML file **with environment-variable overrides**, opens a SQLite database in **WAL mode**, runs **embedded** migrations at startup, and serves HTTP on the configured bind address. A missing DB file is created; an in-memory URL works (for tests).
- All runtime assets are **embedded in the binary**: HTML templates, static files (CSS, htmx), i18n catalogs, and migrations. No filesystem dependency at runtime beyond the SQLite file and the optional config file.
- Every request renders through a template engine with **no hardcoded UI strings** ([ADR-0001](../adr/0001-language-and-i18n.md)). The active **locale** and **theme** are resolved server-side per request (from cookies, falling back to the configured default locale and to OS `prefers-color-scheme` for theme) and reflected in the rendered HTML (`lang` attribute, optional `data-theme` attribute, translated labels).
- **i18n:** en and fr ship as embedded catalogs; a lookup missing in the active locale falls back to the default locale, then to the key itself (so a missing key is visible, never a crash). Adding a further locale is adding a catalog — no template change.
- The **app shell** (left sidebar with wordmark + nav: Dashboard, Spools, Materials; footer tagline) renders on the index route, per the design handoff §Navigation. The Humidity nav item is intentionally **absent** (deferred slice).
- Serving an unknown static path returns 404; a template render error returns 500 (not a panic).

**Key interfaces:** (behavioral — locate them fresh, names are guidance)
- A **configuration** type exposing server bind, database URL, and default locale, loaded from TOML + prefixed env overrides (env wins over file).
- A **persistence entry point** that, given a database URL, returns a ready connection pool with WAL enabled and all embedded migrations applied. This is the seam later slices' SPI adapters (`MaterialRepository`, `SpoolRepository`, …) build on.
- A **translation catalog** capability: `t(locale, key) -> String` with the fallback chain above, backed by embedded per-locale JSON.
- A **renderer** capability that renders an embedded template for a given locale + theme, exposing a `t(key=…)` function to templates bound to the active locale.
- A **theme** value resolved from a cookie (`auto` | `light` | `dark`; unknown ⇒ auto) producing the `data-theme` attribute (empty for auto).
- An **application state** wiring the pool + renderer + default locale into the HTTP driving adapter, and a router exposing the index route and embedded static serving.

**Acceptance criteria:**
- [ ] `cargo build --workspace` and `cargo test --workspace` are green; `cargo clippy --all-targets --all-features -- -D warnings` is clean; `cargo fmt --all -- --check` passes.
- [ ] `tools/check-domain-purity.sh` prints OK (domain has no framework/IO deps); `tools/check-slice-isolation.sh` passes (no slices yet).
- [ ] A unit test proves env overrides beat TOML for the same config key.
- [ ] An integration test opens an **in-memory** SQLite pool and confirms migrations ran.
- [ ] A render test renders the shell in **fr** (French nav labels present) and in **en**, asserts the `lang` attribute matches, asserts `data-theme="dark"` appears when dark is selected and is **absent** under auto, and asserts **no raw i18n key** (e.g. `nav.dashboard`) leaks into output.
- [ ] An e2e test drives `GET /` through the router: 200 in the default locale; the same route with a `lang=fr` cookie returns French labels.
- [ ] Running the binary against a real SQLite file serves the shell; toggling OS light/dark restyles it; a `lang=fr` cookie switches labels (manual smoke, per the verify skill).
- [ ] All assets (templates, static, i18n, migrations) are embedded — the binary serves them with no extra files present besides the DB.

**Out of scope:**
- Any domain entity or table — **no** Material/Spool/Location model, no CRUD, no seed. Migration `0001` is an empty seam; domain tables arrive with their slices.
- The Humidity feature and any MQTT (deferred, no sensors).
- Full visual polish of screens beyond the shell; per-screen layouts belong to their slices. Foundation only needs the design **tokens** (light/dark) wired and the sidebar shell.
- A manual theme/locale toggle **widget** (cookie-driven resolution is in scope; the UI control that sets the cookie can land with the shell polish — do not gold-plate it here).
- Authentication, multi-user, backups.

**References:**
- Plan: `docs/superpowers/plans/2026-07-05-foundation.md` (detailed TDD steps)
- Product brief: `docs/product/brief.md`
- Architecture: `docs/architecture.md` · ADRs: `docs/adr/0001-language-and-i18n.md`, `docs/adr/0002-crate-structure.md`
- Design (shell/tokens): `docs/design.md` + `init_assets/design_handoff_filature/README.md`
- Glossary: `docs/glossary.md`
