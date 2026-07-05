# Design — slice `materials`

_Brainstorming output, 2026-07-05. Second slice after foundation. Feeds
`writing-plans` → `agent-brief`._

## Purpose

The **material referential**: the single source of truth for each filament
type's physical & handling properties — density, drying parameters, humidity
Sensitivity, and default nozzle/bed temperatures. Seeded at startup with common
FDM materials, then editable and extensible by the user. Density feeds the
future Remaining Length derivation (spools slice); Sensitivity feeds the derived
humidity threshold (consumed later by the deferred humidity feature). See
[glossary](../../glossary.md) (Material, Sensitivity), [architecture](../../architecture.md)
slice inventory, and design handoff §6 (Référentiel matériaux).

First real domain slice: introduces the first domain entity, the first `dyn
Trait` at the composition root, the first real migration/table, and the first
`sqlx::query!` (commits a `.sqlx/` dir).

## Scope decisions (settled in brainstorming)

- **CRUD = seed + edit + add. No delete.** A fixed built-in seed is inserted at
  startup; the user edits any field and can add new materials (new filaments
  ship over time). Delete is out of scope for v1 (a Spool will reference a
  Material — referential integrity deferred to the spools slice).
- **Identity = ULID.** Primary key is a ULID (shorter than UUID, lexicographically
  sortable). Generated in the **persistence adapter** (`ulid` crate, app layer) at
  insert time; the domain holds an opaque `MaterialId` newtype and stays pure (no
  id/clock dependency). The repo assigns the id on insert and returns the stored
  `Material`. Stored as `TEXT` (26-char ULID string). `name` carries a UNIQUE
  constraint. This makes a future rename safe and the Spool→Material FK stable.
- **Edit interaction = PUT per row.** Per design §6 the table cells are always
  discrete inputs (not click-to-edit). Changing/blurring an input submits the
  whole row via `PUT /materials/:id`; the server validates and returns the
  re-rendered row fragment. One `edit(material)` use case takes the full entity.
- **Add interaction = inline blank row.** A blank input row at the bottom of the
  table posts to `POST /materials`, which appends the new row fragment. (A
  dedicated form may come later "à l'usage" — not now.)

## Domain (`crates/domain/src/materials/`)

- **`model.rs`**
  - `Material { id: MaterialId, name: String, density: Density,
    drying: DryingParams { temp_c, time_h }, sensitivity: Sensitivity,
    nozzle_c: Temperature, bed_c: Temperature }`.
  - `MaterialId(String)` — opaque newtype (ULID string). No generation logic in
    the domain.
  - `NewMaterial` — the same fields **without** `id` (id is assigned by the repo
    on insert). `add`/seed construct this; the repo returns a full `Material`.
  - Validated newtypes: `Density` ( > 0 g/cm³ ), `Temperature` ( ≥ 0 °C ),
    drying `time_h` ( ≥ 0 ). Invalid values → `DomainError` variants (extend the
    shared error enum).
  - `Sensitivity { Low, Medium, High }`.
- **Derived (pure, tested):** `Sensitivity::humidity_threshold_pct() -> u8`
  → `Low = 40`, `Medium = 30`, `High = 15`. Read-only; drives the referential
  screen's "Seuil %HR" column and the future humidity feature. Threshold rule
  lives here, not in the web layer.
- **`ports/api.rs` — `MaterialsUseCases`:**
  - `list() -> Vec<Material>`
  - `add(NewMaterial) -> Result<Material, …>`
  - `edit(Material) -> Result<Material, …>`
  - `seed_defaults()` — idempotent insert-if-absent of the built-in set.
- **`ports/spi.rs` — `MaterialRepository`:**
  - `list() -> Vec<Material>`
  - `insert(NewMaterial) -> Material` (assigns the ULID)
  - `update(Material) -> Material`
  - `exists_by_name(name) -> bool` (seed idempotence + uniqueness guard)
  - Async trait; **every method is fallible** (`Result<_, RepositoryError>` — a
    domain-side error type, not `sqlx::Error`). Signatures reference **only**
    domain types, never SQLx rows. `insert` maps a UNIQUE(name) violation to a
    distinct error variant so `add` can surface "name already exists".
- **`mod.rs`** — use-case impls over the SPI. `seed_defaults()` walks the
  built-in list and inserts each material absent by name (idempotent, safe to run
  every startup). Unit-tested through a hand-written SPI stub (behind the `stubs`
  feature, per architecture).

## App / adapters (`crates/app`)

- **`persistence/materials.rs`** — `SqlxMaterialRepository` implementing
  `MaterialRepository`. Generates the ULID (`ulid` crate) at insert. Uses
  `sqlx::query!` / `query_as!` (compile-time-checked → commits `.sqlx/`
  metadata). Maps rows ↔ domain types (parses `sensitivity` text ↔ enum).
- **Migration `0002_materials.sql`** — embedded, run at startup after `0001`:
  ```sql
  CREATE TABLE materials (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL UNIQUE,
    density      DOUBLE PRECISION NOT NULL,
    drying_temp_c  INTEGER NOT NULL,
    drying_time_h  INTEGER NOT NULL,
    sensitivity  TEXT NOT NULL,          -- 'Low' | 'Medium' | 'High'
    nozzle_c     INTEGER NOT NULL,
    bed_c        INTEGER NOT NULL
  );
  ```
  (Threshold is derived, never stored.)
- **`web/`** — driving adapter:
  - `GET /materials` → full editable table (`materials.html`).
  - `PUT /materials/:id` → validate + persist row, return re-rendered
    `_material_row.html` fragment (htmx swap).
  - `POST /materials` → insert from the blank row, return the new row fragment
    (appended); a fresh blank row is re-served.
  - Columns per design §6: Matériau (badge) · Densité · Séchage (temp + temps) ·
    Sensibilité (select, coloured green/amber/red) · Seuil %HR (derived,
    read-only) · Buse · Plateau. Discrete inputs (surface bg, accent on focus).
  - No hardcoded UI strings — i18n en/fr catalogs extended. Domain errors mapped
    to localised messages in the web layer (i18n never reaches the domain).
- **`main.rs`** — composition root: build `SqlxMaterialRepository`, wrap as
  `Arc<dyn MaterialRepository>` (the first `dyn Trait`), inject into
  `MaterialsUseCases`; call `seed_defaults()` after migrations, before serve;
  mount the `/materials` routes. The "Materials" nav item already exists in the
  shell.

## Built-in seed (14 materials — all editable/extensible afterwards)

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

Starting points; the user tunes per brand. Seed is idempotent (insert-if-absent
by name), so it never overwrites an edited material on restart.

## Testing (by layer)

- **Domain unit** — `humidity_threshold_pct` per Sensitivity; newtype validation
  (density > 0, temps ≥ 0); `seed_defaults` idempotent and complete via SPI stub.
- **SPI integration** — `insert` assigns a ULID and round-trips; `update`
  persists; `list` returns inserted rows; `exists_by_name`; UNIQUE(name) rejects
  a duplicate — against real PostgreSQL via testcontainers.
- **Render** — table + single row rendered in **en** and **fr**: no raw i18n key
  leaks, derived "Seuil %HR" shown, Sensitivity coloured.
- **e2e** (through Axum): `GET /materials` → 200 with seeded rows; `PUT` a
  changed density → 200, fragment reflects it; `POST` a new material → 200,
  new row present.

## Out of scope

- Delete of a material.
- Spool → Material FK / referential integrity (spools slice).
- Consumption of the humidity threshold by any humidity UI (deferred, no
  sensors).
- A dedicated add **form** widget (inline blank row only for now).

## References

- Glossary: `docs/glossary.md` (Material, Sensitivity, Humidity Threshold)
- Architecture: `docs/architecture.md` (slice inventory, key constraints)
- ADRs: 0001 (EN code / i18n UI), 0002 (crate structure), 0003 (PostgreSQL)
- Design handoff §6: `init_assets/design_handoff_filature/README.md`
- Prior slice: `docs/specs/foundation.md`
