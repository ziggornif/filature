# Architecture

> High-level overview: topology, key structural decisions, context for ADRs.
> Hard-to-reverse, surprising, trade-off decisions go in `docs/adr/` instead.

Filament stock manager. Self-hosted binary backed by PostgreSQL
([ADR-0003](adr/0003-postgresql-persistence.md); supersedes the brief's SQLite).
Hexagonal (ports & adapters) with vertical slices. Stack: Axum, SQLx (Postgres),
Tera, htmx, Tokio. Terms are the glossary's canonical English names.

## Topology

Two-crate Cargo workspace — the crate graph is the compiler-enforced dependency
rule (see [ADR-0002](adr/0002-crate-structure.md)):

```
crates/
├── domain/            # zero framework/IO deps. Pure. Compiler forbids the rest.
│   └── src/
│       ├── shared/            # minimal kernel: identifiers, units, Money-ish, errors
│       ├── materials/         # slice: material referential
│       │   ├── mod.rs         # use cases (impl the API port)
│       │   ├── model.rs       # Material, Sensitivity, drying params
│       │   └── ports/{api.rs, spi.rs}
│       ├── spools/            # slice: the richest domain — weight/length/status/value
│       ├── locations/         # slice: storage places (no sensor in v1)
│       ├── dashboard/         # slice: read-only aggregates
│       └── ports/spi/stubs/   # hand-written SPI test doubles, behind `stubs` feature
└── app/               # composition root + all adapters + the single binary
    └── src/
        ├── main.rs            # wires SQLx repos (dyn Trait) into use cases, builds router
        ├── config.rs          # TOML + env overrides
        ├── web/               # DRIVING adapter: Axum handlers, Tera, htmx fragments, DTOs
        │   ├── i18n/          # translation catalogs (en, fr, …) + locale selection
        │   └── theme.rs       # light/dark cookie (design handoff)
        ├── persistence/       # DRIVEN adapter: SQLx repos implementing the SPI traits
        └── assets/            # embedded templates, htmx, CSS, fonts, migrations
```

Single binary holds: embedded migrations + static assets (CSS, self-hosted woff2
fonts, templates) + translation catalogs. **htmx is loaded from a CDN** (jsdelivr,
with SRI integrity) rather than embedded — the one deliberate runtime network
dependency on the frontend. Persistence is **PostgreSQL** (external service, the
only backend dependency; [ADR-0003](adr/0003-postgresql-persistence.md)). Config =
TOML file, env overrides. Backup = `pg_dump` / managed Postgres.

## Key constraints

- **Domain depends on nothing.** No Axum/SQLx/Tera/serde/tokio in `crates/domain`.
  Enforced by the crate graph + `tools/check-domain-purity.sh` (allowlist:
  `thiserror`, `rust_decimal` for money/weights, maybe `time`). i18n never
  reaches the domain — locales are a `web/` concern; the domain speaks domain
  objects and domain errors, the web adapter maps them to localised strings.
- **Ports are traits.** `ports::api` (driving, one per slice) and `ports::spi`
  (driven — a narrow repository trait per slice). SPI signatures reference only
  domain objects, never SQLx rows or DTOs.
- **`dyn Trait` at the composition root.** `main.rs` injects concrete SQLx repos
  as trait objects into the use cases. Generics only if a path ever justifies it.
- **Time/IDs via SPI.** The domain needs "opened at" timestamps and new spool
  ids without touching the clock or DB directly → `Clock` and `IdGenerator` SPIs
  in `shared/ports/spi` (or IDs from the repository on insert — decided per slice
  in its brief). No `now()`/`uuid()` inside the domain.
- **Derived values are pure domain.** Remaining Ratio, Remaining Length (needs
  Material density + Spool diameter), Stock Value, low-stock signalling, Spool
  Status transitions — all computed in `crates/domain`, no I/O. This is where the
  real behaviour lives; entities carry it, not the handlers.
- **SQLx compile-time checks offline.** `sqlx::query!` verified against checked-in
  `.sqlx/` metadata so the build needs no live DB; migrations embedded and run at
  startup.

## Slice inventory

| Slice | Responsibility | API port (use cases) | SPI it declares |
|---|---|---|---|
| `materials` | Material referential — source of truth for density, drying, Sensitivity | list, edit, seed-on-startup | `MaterialRepository` |
| `spools` | Spool lifecycle & all derived quantities (the domain-heavy slice) | add, edit, list (filter/sort), view, adjust-weight (weigh / consume), archive | `SpoolRepository`, `Clock`, `IdGenerator` |
| `locations` | Storage places (plain in v1; Drybox specialisation deferred) | list, add, edit | `LocationRepository` |
| `dashboard` | Read-only aggregates over spools/materials | view (value, remaining, split by material, soon-empty) | its own `StockOverviewRepository` (aggregate reads) |

No slice imports another slice. The dashboard does **not** reuse `spools`'
`SpoolRepository`; it declares its own narrow `StockOverviewRepository` in its
own `ports/spi`. A single SQLx adapter struct in `crates/app` implements several
slices' SPI traits and is wired to each at the composition root — grouping
adapters by actor while keeping ports segregated per slice. Cross-slice domain
types (a Spool referencing its Material) live in `shared/`, never a direct
slice→slice `use`.

## Composition root

`crates/app/src/main.rs`: load config → connect Postgres pool → run embedded
migrations → seed materials → build the SQLx repo structs → inject them (as
`Arc<dyn …Repository>`) plus `Clock`/`IdGenerator` into each slice's use cases →
build the Axum router (driving adapter) with the Tera engine, i18n middleware,
theme cookie, and embedded static assets → serve. No background task in v1 (the
MQTT humidity ingestion task is deferred).

## Deferred (post-v1, no sensors)

Humidity: a `humidity` slice (`HumidityReading`, `Drybox Status`) + a driven MQTT
ingestion task (Tokio, `rumqttc`) writing readings through an SPI, + a driving
humidity panel polling every 60s. Locations already carry the seam (a Location
becomes a Drybox when a topic + sensor exist). Material Sensitivity/threshold
fields are stored now as the future source of truth. See the brief scope note.

## Testing strategy

Maps to the layers (see the hexagonal skill):
- **Domain unit tests** — pure, no mocks: length↔weight, ratio, status transitions
  (remaining→0 ⇒ Empty), stock value, low-stock threshold.
- **Use-case tests** — through SPI stubs in `crates/domain` (`stubs` feature).
- **Persistence (SPI) integration tests** — against a real PostgreSQL via
  **Testcontainers** (Docker), per slice — test the engine you ship
  ([ADR-0003](adr/0003-postgresql-persistence.md)).
- **Template render tests** — every Tera page + htmx fragment rendered with a
  representative context, in the default and at least one non-default locale, so
  missing i18n keys and Tera variable typos fail at `cargo test` (brief §6 +
  ADR-0001). Do not skip.
- **e2e** — through the Axum adapter on the critical journeys only (add spool →
  detail; inline weight adjust → status flips).
