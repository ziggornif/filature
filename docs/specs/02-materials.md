## Agent Brief

_AI-generated brief. Origin: orchestrator harness, phase 4. Brainstorming
decisions and the detailed TDD plan folded into this contract; no separate
design/plan file is kept (harness convention — see `CLAUDE.md`)._

**Category:** feature
**Summary:** Build the `materials` slice — the filament **Material** referential (density, drying, **Sensitivity**, default nozzle/bed temperatures), seeded at startup, listable, editable, and extensible through an htmx table.

**Slice / context:** Second slice, first real domain, building on the foundation
substrate (app boots → Postgres pool → embedded migrations → app-shell in en/fr,
light/dark). Foundation introduced **no** domain entity; this slice introduces the
first: the `Material` entity, the first outbound SPI port and its first
`Arc<dyn …>` wiring at the composition root, the first real table/migration, and
the first compile-checked `sqlx::query!` (commits a `.sqlx/` dir). `Material` is
the source of truth for density (feeds the future Remaining Length in `spools`)
and Sensitivity (feeds the derived humidity threshold, consumed later by the
deferred humidity feature). See glossary (Material, Sensitivity, Humidity
Threshold) and design handoff §6 "Référentiel matériaux".

**Desired behavior:**
- On startup the referential is **seeded** with a built-in set of 14 common FDM
  materials (table below). Seeding is **idempotent** — insert-if-absent by name —
  so restarting never overwrites a value the user has edited.
- The user can **list**, **edit**, and **add** materials. **No delete** in v1 (a
  Spool will reference a Material; referential integrity is deferred to `spools`).
- **Identity is a ULID**, generated in the persistence adapter at insert time and
  returned by the repository; the domain holds an opaque identifier and never
  generates ids or reads a clock. `name` is **unique**; adding a duplicate name
  is rejected with a distinct, surfaceable error (not a 500).
- Each material carries: `name`, `density` (g/cm³, must be **> 0**), drying
  parameters (temperature °C + time h), `Sensitivity` (**Low | Medium | High**),
  default nozzle °C, default bed °C.
- The **humidity threshold** (%RH) is a **pure derived value** of Sensitivity —
  `Low=40, Medium=30, High=15` — computed in the domain, shown read-only in the
  table, **never stored**.
- The **screen** is one editable table, one row per material, columns per design
  §6: Material (badge) · Density · Drying (temp + time) · Sensitivity (select,
  coloured green/amber/red) · RH threshold (derived, read-only) · Nozzle · Bed.
  Editing a row persists it and returns the re-rendered **row fragment** (htmx
  swap). Adding uses a **blank input row** at the bottom that appends the new
  row fragment on submit.
- **No hardcoded UI strings** (ADR-0001): every label localised en/fr; a domain
  error is mapped to a localised message in the web layer — **i18n never reaches
  the domain**. Adding a locale is adding a catalog, no template change.
- Invalid input (density ≤ 0, unknown Sensitivity) is rejected without a panic
  and without persisting; the user-facing response is a client error, not 500.

**Key interfaces:** (glossary + API/SPI terms — names are guidance, no file paths)
- `Material` — the referential entity: identifier, name, density, drying params,
  Sensitivity, nozzle/bed defaults. `NewMaterial` — the same **without** the
  identifier (assigned by the repository on insert).
- `Sensitivity` — enum `Low | Medium | High`; exposes the pure derived
  `humidity_threshold_pct` (40 / 30 / 15) and string round-trip for persistence.
- Validated value types: a density that refuses non-positive values; a
  temperature carrying meaning for nozzle/bed/drying temp.
- `MaterialsUseCases` (**API port**): `list`, `add(NewMaterial)`,
  `edit(Material)`, `seed_defaults` — all async, fallible.
- `MaterialRepository` (**SPI port**): `list`, `insert(NewMaterial) -> Material`
  (assigns the id), `update(Material)`, `exists_by_name(name)` — all async,
  fallible. A domain-side `RepositoryError` distinguishes a **duplicate name**
  from a generic backend failure; signatures reference only domain types, never
  SQLx rows or DTOs. The concrete SQLx adapter generates the ULID and maps a
  UNIQUE violation to the duplicate variant.
- Composition root injects `Arc<dyn MaterialRepository>` into the use cases and
  calls `seed_defaults` after migrations, before serving. Async SPI uses
  `async-trait` (a proc-macro, filtered by the domain-purity sensor — allowed in
  the domain without an allowlist change).

**Built-in seed (14 materials — all editable/extensible afterwards):**

| Material | Density g/cm³ | Dry °C | Dry h | Sensitivity | Nozzle °C | Bed °C |
|---|---|---|---|---|---|---|
| PLA     | 1.24 | 45 | 6 | Low    | 210 | 60  |
| PLA-CF  | 1.30 | 45 | 6 | Low    | 220 | 60  |
| PETG    | 1.27 | 65 | 6 | Medium | 240 | 80  |
| PETG-CF | 1.30 | 65 | 6 | Medium | 250 | 80  |
| ASA     | 1.07 | 70 | 4 | Medium | 250 | 100 |
| ABS     | 1.04 | 70 | 4 | Medium | 245 | 100 |
| HIPS    | 1.04 | 65 | 4 | Medium | 240 | 100 |
| PP      | 0.90 | 60 | 4 | Low    | 230 | 85  |
| TPU     | 1.21 | 55 | 6 | High   | 225 | 40  |
| PVA     | 1.23 | 45 | 6 | High   | 200 | 60  |
| PA      | 1.14 | 80 | 8 | High   | 260 | 90  |
| PA-CF   | 1.16 | 80 | 8 | High   | 270 | 90  |
| PA-GF   | 1.20 | 80 | 8 | High   | 270 | 90  |
| PC      | 1.20 | 90 | 6 | High   | 270 | 110 |

**Acceptance criteria:**
- [ ] `Sensitivity::humidity_threshold_pct` returns 40/30/15 for Low/Medium/High (domain unit test).
- [ ] Density construction rejects `0.0` and negatives; Sensitivity parsing rejects an unknown string — both as domain errors, tested.
- [ ] `seed_defaults` inserts all 14 built-ins; running it a second time leaves the count unchanged (idempotent) — tested through the SPI stub.
- [ ] `add` with an already-present name returns the **duplicate** error, not a generic failure — tested through the stub and against real Postgres.
- [ ] Against **real PostgreSQL** (Testcontainers): `insert` assigns a 26-char ULID and round-trips all fields; `update` persists; `exists_by_name` is correct; a UNIQUE(name) violation surfaces as the duplicate error.
- [ ] Migration `0002` creates the `materials` table (threshold is **not** a column); it runs after `0001` at startup; the build works offline against checked-in `.sqlx/` metadata.
- [ ] A render test renders the table and a row fragment in **en** and **fr**: the derived RH threshold is shown, Sensitivity is present, and **no raw i18n key** leaks.
- [ ] e2e through the router: `GET /materials` returns 200 listing seeded rows (e.g. contains "PLA" and "PA-CF"); `POST /materials` with a valid form adds a material and returns its row fragment.
- [ ] `cargo test --workspace --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`, `tools/check-domain-purity.sh`, and `tools/check-slice-isolation.sh` all pass; no `docs/superpowers/` tree.
- [ ] Manual smoke (verify skill): startup seeds the table; the Materials screen lists them; editing a density and changing a Sensitivity persist and re-render the row; adding a material via the blank row appends it; labels switch with a `lang=fr` cookie.

**Out of scope:**
- **Delete** of a material.
- Spool → Material FK / referential integrity (the `spools` slice).
- Any humidity UI or consumption of the RH threshold beyond displaying it (deferred; no sensors).
- A dedicated add **form** widget (inline blank row only for now — revisit "à l'usage").
- Filtering/sorting the referential, bulk import, authentication.

**References:**
- Glossary: `docs/glossary.md` (Material, Sensitivity, Humidity Threshold)
- Architecture: `docs/architecture.md` (slice inventory, key constraints)
- ADRs: `docs/adr/0001-language-and-i18n.md`, `docs/adr/0002-crate-structure.md`, `docs/adr/0003-postgresql-persistence.md`
- Design (screen §6): `docs/design.md` + `init_assets/design_handoff_filature/README.md`
- Prior slice: `docs/specs/01-foundation.md`
