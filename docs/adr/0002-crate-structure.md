# ADR-0002 — Two-crate workspace, domain isolated

Status: accepted (2026-07-05)

## Context / forces

The source brief (`init_assets/BRIEF-filature.md` §3) called for **one crate**
for the MVP — "hexagonal léger", extract the domain later if needed. The harness
Rust reference argues the opposite: isolating the domain in its own crate makes
the dependency rule **compiler-enforced** (the domain crate simply cannot name
Axum/SQLx/Tera because they aren't in its `Cargo.toml`), which is the cheapest
and strongest architectural sensor available. A single crate loses that; purity
then rests only on a grep-level script, which is weaker and easy to bypass.

Constraint that is *not* in tension: the single-binary requirement. A Cargo
workspace still produces one binary (built by the `app` crate); multiple crates
≠ multiple artifacts.

## Decision

A **two-crate workspace**:
- `crates/domain` — pure. Modules per slice (`materials`, `spools`, `locations`,
  `dashboard`), `shared/` kernel, `ports::{api,spi}`, SPI stubs behind a feature.
  Dependency allowlist only (`thiserror`, `rust_decimal`, `time`).
- `crates/app` — composition root + all adapters (Axum `web/`, SQLx
  `persistence/`, config, i18n, embedded assets) + `main.rs`, the single binary.

Not one crate (brief), not four (domain + adapter-sqlx + adapter-web + app).

## Rejected alternatives

- **Single crate with internal modules (the brief's choice).** Simplest, but the
  compiler no longer enforces domain purity — only `tools/check-domain-purity.sh`
  does, at module granularity. Weaker sensor for little ceremony saved. Rejected:
  isolating one extra crate is cheap and buys compiler enforcement.
- **Full four-crate split (domain + adapter-sqlx + adapter-web + app).** Maximal
  isolation but real ceremony (crate boundaries between the two adapters and the
  root) with little payoff at this size. Deferred: promote `web` and
  `persistence` to their own crates only if the app crate grows unwieldy.

## Consequences

- Domain purity is compiler-enforced for the framework/IO boundary; the allowlist
  script (`tools/check-domain-purity.sh`) is a backstop, not the primary line.
- Adapters live together in `crates/app`, wired at `main.rs` via `dyn Trait`.
  Splitting them later is a mechanical move if needed.
- Deviates from the brief deliberately; the brief's "extract later" intent is
  honoured earlier because the cost is low and the sensor value is high.
- Slice isolation *inside* `crates/domain` is not compiler-enforced (same crate);
  a grep-level slice-isolation check covers it (per the Rust reference).
