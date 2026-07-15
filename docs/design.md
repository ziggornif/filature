# Design

> Decisions about the product's interface and aesthetic character.
> **Source of truth for the UI is the hi-fi handoff** in
> `init_assets/design_handoff_filature/` (README.md = spec, Filature.dc.html =
> visual reference, `support.js` = proto runtime, NOT an implementation target).
> This file records the load-bearing decisions and the deltas from that handoff.
> No prototype UI branch was run — the design is already hi-fi.

## Character

A **workshop instrument** kept open in a tab, not a consumer app. Dense,
legible, calm. The only vivid colour comes from the data (each spool's real
filament colour); semantic colour (green/amber/red) is reserved for status,
never decoration.

## Visual language (from the handoff — treat as definitive)

- **Themes light AND dark at parity.** Default = OS `prefers-color-scheme`, plus
  a persistent manual toggle. CSS tokens on `:root`, overridden via
  `html[data-theme="light|dark"]`; the attribute is set **server-side at render**
  from a cookie so the OS default is honoured on first paint.
- **Palette:** neutral warm greys (workshop, not clinical blue), 3 surface
  levels. One discreet accent (`#5b5563` slate default). Semantic tokens
  (`--ok`/`--warn`/`--danger`) with a per-theme variant. Full token values (light
  + dark) are in the handoff README §Design Tokens — copy them verbatim.
- **Typography:** IBM Plex Sans (UI) + IBM Plex Mono (all numbers, units, codes).
  **Monospace for every figure** (g, m, %, %HR, °C, €) to align columns — the
  instrument feel. Self-host the woff2 (embedded in the binary, no network).
- **Signature components:** filament colour **chip** (always a
  `--border-strong` ring; hatch pattern for "transparent"); **remaining-weight
  gauge** (bar whose fill goes neutral → amber under low threshold → red under
  10% → grey when empty, with g + % in mono).
- **Icons:** Feather/Lucide light set, inline SVG.
- **Radii/spacing:** cards 11–12px, controls 6–8px, chips circular, status pills
  20px; grid gap 14px. Exact values in the handoff.

## Screens (6, specified in the handoff README §Screens)

Dashboard · Spools list (table + card views, inline weight edit) · Spool detail ·
Add/Edit form (**wizard 2 écrans** — état → détails ; poids net par presets, **pas de tare**) · Materials table ·
**Humidity panel (deferred — post-v1, no sensors).**

## Interaction principles (htmx, server-rendered)

- Every self-updating unit is an **autonomous htmx fragment** re-rendered in
  place (a spool row, a card, the list panel). No SPA, no custom JS beyond htmx,
  no build step.
- **Filtering** = each control `hx-get` → swaps the `<tbody>`/grid (light debounce
  on search). **Inline weight edit** = `hx-get` edit fragment → `hx-put` returns
  the re-rendered row; Enter commits, Esc cancels; remaining→0 flips status to
  Empty. **Theme** = cookie + `data-theme` on `<html>`.
- State lives server-side; handlers return HTML fragments. The proto's in-memory
  JS state is NOT ported.

## Deltas from the handoff

- **htmx via CDN (not embedded).** htmx is loaded from jsdelivr with an SRI
  `integrity` hash + `crossorigin`, rather than vendored into the binary. This is
  a deliberate runtime network dependency on the frontend (trades offline
  self-sufficiency for a lighter binary + browser CDN caching). Self-hosted woff2
  fonts stay embedded; only htmx is CDN-loaded.
- **i18n (ADR-0001).** The handoff assumed a French UI; the real UI is
  internationalised (en + fr shipped, extensible). No hardcoded strings — every
  user-facing label comes from a per-locale catalog. htmx fragments must render
  in the active locale (locale resolved server-side, like the theme). Render
  tests cover a non-default locale so missing keys fail at `cargo test`.
- **Humidity screen deferred.** Present in the handoff (screen 3); out of v0-v1
  scope (no sensors). Build the other 5 screens; leave the humidity nav item /
  panel for the deferred slice.

## Responsive (from the handoff)

Desktop-first. ≤1040px: KPIs 2-col, dashboard sections stacked. ≤760px: sidebar →
60px icon rail (theme toggle hidden, follows OS), everything 1-col, spool table
horizontally scrollable. Dashboard stays fully legible (requirement). Reproduce
the proto's CSS-var breakpoints as real media queries.

## Out of scope (design)

No illustrations/imagery (the only "visual" is data colour). No drag-and-drop.
At most one simple modal. No custom charting beyond the 24h humidity sparkline
(deferred with humidity). No design work on the deferred humidity screen beyond
what the handoff already specifies.
# Cross-slice orchestration: spool auto-unload

When a web operation makes a spool Empty (remaining weight reaches zero) or
Archived, the app-crate handler calls `PrintersUseCases::unload_spool` after
the successful spool mutation. This is an intentional edge-orchestration seam:
the `spools` and `printers` domain slices remain independent, while the
driving adapter coordinates the two use cases. A database trigger was rejected
because it would hide this behaviour and make it harder to test. If more
cross-slice reactions accumulate, a domain-event/outbox design should replace
this explicit seam.
