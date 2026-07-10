## Agent Brief — 05 Dashboard (stock-at-a-glance overview)

> AI-generated brief (harness `agent-brief`). Reviewer: treat as authoritative spec; verify against glossary + code, not against original discussion.

**Category:** feature
**Summary:** Introduce the read-only `dashboard` slice — the landing screen (`GET /`) showing stock at a glance: 4 KPI cards (Stock Value, remaining weight, spool counts, low-stock alert count), a per-material breakdown, and a "soon-empty" short list. No mutation, no htmx interactivity — a single server-rendered page computed on load.

**Slice / context:**
Fifth vertical slice. Foundation, `materials` (02), `spools` core+ops (03a/03b) and
`locations` (04) are shipped and merged. This slice **only reads** — it aggregates the
existing spool/material stock into an overview; it adds no new aggregate, no write path,
and no new domain invariant on existing types. It serves job #3 of the product brief
("See stock at a glance") and realises screen §1 of the design handoff
(`init_assets/design_handoff_filature/README.md` — "Tableau de bord").

Today `GET /` renders a static `index.html` placeholder (see the `index` handler); this
slice replaces that landing page with the real dashboard. The **Stock Value** aggregate
already exists end-to-end: `SpoolsUseCases::stock_value` / `SpoolRepository::stock_value`
return `Money = Σ (remaining/net × price_paid)` over non-archived spools (03b). Reuse that
semantics; do not recompute it differently.

**Scope decisions already settled (product calls, 2026-07-10):**
- **Low-stock threshold = Remaining Ratio ≤ 0.15 AND Remaining Weight > 0.** A spool at
  exactly 0 g is `Empty` (already finished), **not** "soon-empty" — it is excluded from
  both the Alerts count and the Soon-empty list.
- **Alerts KPI card is kept**, its value = the low-stock count above. (The design's
  original "+ dryboxes in red" term is **deferred** with humidity — the card counts
  low-stock spools only in v1.)
- **Humidity section (design §1.3) is omitted entirely** — no markup, no placeholder.
  Reintroduced with the deferred `humidity` slice ([[filature-humidity-deferred]]).

**Desired behavior:**

Rendering `GET /` produces one HTML page (localised en/fr, theme-aware, using the app
shell + sidebar, "Tableau de bord" nav item active) with three regions. Everything is
computed over **non-archived spools only** (Archived spools are out of active stock, as
in `stock_value` and the list).

1. **Four KPI cards:**
   - **Stock Value** — `Money`, Σ (remaining/net × price_paid) over non-archived spools.
     Same value `stock_value` already returns for an empty filter.
   - **Remaining weight** — Σ Remaining Weight over non-archived spools, shown in kg.
   - **Spools** — total count of non-archived spools, plus a breakdown of **active**
     (`Sealed` + `Open`) vs **empty** (`Empty`).
   - **Alerts** — count of **low-stock** spools (ratio ≤ 0.15 and remaining > 0),
     rendered in the danger style.
2. **Material breakdown** — one row per Material that has **≥1 non-archived spool**:
   material name, its non-archived spool count, its summed Remaining Weight (kg), and a
   proportional mini-bar (width = this material's weight ÷ the largest material's weight).
   Materials with zero non-archived spools do not appear.
3. **Soon-empty list** — the low-stock spools (ratio ≤ 0.15 and remaining > 0), **sorted
   by Remaining Ratio ascending** (closest to empty first). Each entry shows the colour
   chip, a material/colour label, the assigned Location name (or an "unassigned" label),
   and the Remaining Weight in g with the Remaining Ratio as a %, coloured when low. Each
   entry links to that spool's detail view. When there are no low-stock spools the region
   shows an empty-state message (not a broken/empty box).

**Edge cases:**
- **No spools at all** (or all archived): Stock Value = 0, remaining = 0 kg, spool counts
  all 0, alert count 0, material breakdown empty (empty-state), soon-empty empty
  (empty-state). The page must render cleanly, never divide-by-zero (mini-bar and ratio
  guard the zero-max / zero-net cases).
- A spool whose Net Weight makes ratio computation degenerate must not panic — reuse the
  existing `Remaining Ratio` derivation (`Grams::ratio_of`), which already handles this.
- Threshold is an **inclusive** boundary: ratio exactly 0.15 **is** low-stock.

**Key interfaces:** (glossary + API/SPI terms — no file paths; agent explores fresh)
- New vertical slice `dashboard` in the domain crate, mirroring the existing slice layout
  (model/read-models, `ports/api`, `ports/spi`, usecases, stubs). Read-only: no aggregate,
  no mutating use case.
- **API port** — a `DashboardUseCases`-style trait exposing a single read operation
  returning a `DashboardOverview` read model. Input: none (or an all-stock scope).
- **`DashboardOverview` read model** — carries the computed KPIs, the material-breakdown
  rows, and the soon-empty rows. Following the established convention
  (see `SpoolListItem`), read models carry cross-slice fields (`material_name`,
  `location_name`) as **plain primitives**; the `dashboard` slice must **not** import the
  `materials` or `spools` slices' internals — any cross-table join happens in the SQLx
  adapter.
- **SPI port** — a `DashboardRepository`-style outbound trait the adapter implements to
  supply the raw stock data the overview is computed from. Whether the low-stock
  threshold / grouping is applied as SQL aggregates in the adapter or as a pure fold in
  the domain is the implementer's call — **but the 0.15 threshold is a domain rule**: it
  must live as a named domain constant and be **unit-testable in the domain layer without a
  database** (mirroring how the materials 40/30/15 thresholds are pure-derived, never
  stored). Prefer computing the KPIs/groupings/filtering in the domain from primitives so
  the rule is testable there; the SPI stays a thin data-supply port.
- Reuse `Money`, `Grams` (`ratio_of`), `SpoolStatus`, `MaterialId` from `shared`/existing
  slices. Do **not** duplicate the `stock_value` SQL — reuse its definition/semantics.
- **Driving adapter** — the `GET /` route now renders the dashboard via a new web handler
  + Tera template(s), replacing the `index.html` placeholder. htmx is **not** required
  (static render); do not add polling or fragment routes.

**Acceptance criteria:**
- [ ] `GET /` returns the dashboard page (200, HTML), app shell + sidebar with "Tableau de
      bord" active; the old `index.html` placeholder is gone.
- [ ] Stock Value KPI equals `Σ (remaining/net × price_paid)` over non-archived spools and
      matches what `stock_value` returns for the all-stock scope (assert equality in a test).
- [ ] Remaining-weight KPI equals the sum of Remaining Weight over non-archived spools,
      displayed in kg.
- [ ] Spools KPI shows total non-archived count and the active (`Sealed`+`Open`) / empty
      (`Empty`) split; Archived spools are excluded from every count.
- [ ] Alerts KPI equals the count of spools with Remaining Ratio ≤ 0.15 and Remaining
      Weight > 0; a spool at 0 g is excluded; a spool at exactly ratio 0.15 is included.
      Covered by a domain unit test on the threshold rule (boundary + zero cases).
- [ ] Material breakdown lists exactly the materials with ≥1 non-archived spool, each with
      correct spool count and summed weight; the mini-bar width is proportional to the max
      material weight; a single-material stock gives that material a full-width bar.
- [ ] Soon-empty list contains exactly the low-stock spools (ratio ≤ 0.15, remaining > 0),
      sorted by ratio ascending, each showing colour/material label, location name or
      "unassigned", remaining g and ratio %; each links to the spool detail.
- [ ] Empty stock (no spools / all archived) renders the page with all zeros and
      empty-state messages for both regions — no panic, no divide-by-zero.
- [ ] The 0.15 threshold exists as a named domain constant and is exercised by a
      database-free domain unit test.
- [ ] i18n en + fr parity for every new UI string (no hardcoded UI text); theme-aware.
- [ ] SPI adapter covered by an integration test (testcontainers, ADR-0003) proving the
      overview is populated from real rows (KPIs, breakdown, soon-empty) with archived
      spools excluded.
- [ ] Domain-purity check passes (no I/O/framework types in `crates/domain`); clippy +
      offline build clean; full suite green.

**Out of scope:**
- Humidity / drybox section (design §1.3) — deferred, no markup or placeholder.
- Any write/mutation, htmx interactivity, live polling, or fragment routes — the page is a
  static server render.
- New domain invariants or changes to existing `Spool`/`Material`/`Location` aggregates,
  ports, or persistence beyond the read-only aggregation this slice needs.
- Per-job consumption, per-print cost, export, auth — out per the product brief firewall.
- Re-deciding Stock Value semantics or the no-tare Net Weight model (ADR-0004).
- Touching the existing accepted-debt items (TD-003/005/006/007/008/009/010) — log, don't
  fix, unless one is directly in the read path this slice adds.

**References:**
- Product brief: `docs/product/brief.md` (job #3 "See stock at a glance"; scope: dashboard)
- Design: `docs/design.md` + `init_assets/design_handoff_filature/README.md` §1 "Tableau de bord"
- Glossary: `docs/glossary.md` (Stock Value, Remaining Ratio, Remaining Weight, Spool Status)
- ADRs: `docs/adr/` — ADR-0003 (PostgreSQL/testcontainers), ADR-0004 (Net Weight, no tare)
- Prior slices as structural template: `docs/specs/03a-spools-core.md`, `docs/specs/04-locations.md`
