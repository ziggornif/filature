# ADR-0003 — PostgreSQL for persistence (supersedes the brief's SQLite choice)

Status: accepted (2026-07-05)

## Context / forces

The source brief (`init_assets/BRIEF-filature.md`) fixed **SQLite** as the store:
"un seul binaire + un fichier SQLite", WAL mode, backup = file copy, k3s deploy of
just the binary. On review of the foundation PR the user clarified the real
deployment intent: **the deployed environment will run PostgreSQL**, and the
"self-sufficient single binary" constraint is meant to apply to *everything
except the database*. The DB is explicitly allowed to be an external service.

This reverses a firm brief decision, so it is recorded here rather than silently
changed. Foundation is the cheapest moment to switch: no repository/SPI adapter
or domain table exists yet — only the connection-pool + migration seam.

## Decision

Use **PostgreSQL** as the persistence engine, via SQLx (`postgres` feature).

- Config `database.url` is a `postgres://…` URL.
- Migrations remain embedded (`sqlx::migrate!`) and run at startup, written in
  Postgres SQL.
- No WAL / journal-mode concern (SQLite-specific) — dropped.
- **SPI adapter integration tests run against a real Postgres via Testcontainers**
  (Docker), replacing the in-memory-SQLite approach. CI runs a Postgres service.
- The binary stays self-sufficient for everything else (embedded templates,
  static assets, i18n catalogs, migrations). "Self-sufficient" now means "no
  external dependency **other than the database**".

## Rejected alternatives

- **Stay on SQLite (brief).** Simplest deploy (one file, file-copy backup, no
  service). Rejected: the user's actual production target is Postgres, and
  carrying a SQLite-shaped persistence layer would mean rewriting the adapter and
  every integration test later, at higher cost than doing it now at the seam.
- **SQLite in dev/test, Postgres in prod.** Tempting (fast in-memory tests) but
  it means the integration tests exercise a *different* engine than production —
  the SPI adapter's SQL, types, and migrations would be validated against the
  wrong dialect. Rejected: test the engine you ship.

## Consequences

- `crates/app` depends on `sqlx` with `postgres` (not `sqlite`); dev-dependency on
  `testcontainers` (+ modules for Postgres). Docker required to run integration
  tests locally and in CI.
- CI gains a Postgres service (the `ci.yml` template already anticipated one).
- Backup strategy is now `pg_dump` / managed-Postgres backups, not file copy.
- k3s deployment gains a Postgres dependency (managed instance or a
  statefulset) — the Helm chart must provision/point at it.
- `docs/architecture.md`, `docs/product/brief.md` updated to match. The brief's
  SQLite statements are superseded by this ADR.
- Deferred-humidity note unchanged; the MQTT slice will write readings through an
  SPI to the same Postgres.
