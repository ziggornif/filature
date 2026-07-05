# Technical Debt

> Running log of known debt: deliberate shortcuts, workarounds, and design gaps.
> Each entry: what it is, why it was accepted, cost if left, and a rough priority.
> Resolved entries move to an `## Resolved` section at the bottom rather than being deleted.

## Active debt

| ID | Description | Why accepted | Cost if left | Priority |
|-----|-------------|--------------|--------------|----------|
| TD-001 | **Home-grown i18n** (`web/i18n.rs`): ~40-line JSON-catalog lookup with locale→default→key fallback. No pluralization, gender, or `{var}` interpolation. | Does exactly what ADR-0001 needs today (key→string); zero deps; YAGNI. Reviewed crates: **fluent-templates** (Mozilla Fluent/ICU — plurals, gender, interpolation, `.ftl` files; heaviest, most capable) and **rust-i18n** (compile-time macro, JSON/YAML/TOML catalogs, `t!()` macro; lighter but macro-bound). | When a real string needs a plural or an interpolated value, the custom layer must grow or be swapped — a swap touches every template's `t()` call. | low — revisit when the first pluralized/interpolated string appears (likely a slice with counts, e.g. "N bobines"). |
| TD-002 | **Basic observability**: startup uses `tracing` with a simple subscriber; no structured request spans, no metrics, no log levels wired to config. | Foundation needs *a* real logger (not `println!`); full observability is not needed pre-v1. | Debugging a deployed instance is coarse until request-scoped tracing + levels land. | med — add `tower-http` trace layer spans + config-driven level when the app is actually deployed to k3s. |

## Resolved
<!-- moved here once addressed, with date and how it was resolved -->
