# Product Brief: Filature

> Discovery reframing of `init_assets/BRIEF-filature.md` into problem-space.
> The source brief also fixes stack & architecture — those are recorded in
> `docs/architecture.md` / ADRs, not here. This file is problem, user, outcome, risk.

## Problem

A 3D-printing filament inventory spread across several printers and dryboxes is
impossible to keep in your head: you don't know what's left (weight, length,
value), when a spool was opened, or — the part no tool solves — **whether
humidity-sensitive materials have gone damp**. Damp filament (nylon, PA-CF, PC)
prints badly or fails, and the only signal today is a ruined print. Existing
tools (Spoolman) track stock but ignore humidity, so material state stays
invisible until it costs a job.

## Target user

One concrete person: the homelab owner running several 3D printers with SHT31
humidity sensors already wired into dryboxes over MQTT. Two hats:
- **Hobbyist** at the workbench — keeps a tab open, wants at-a-glance stock and
  a warning before a sensitive spool prints wet.
- **Zig Factory micro-business** — needs material cost of a printed part for
  quoting (later phase).

Not a general/public audience — a single self-hosted operator. This is a
**workshop instrument**, not a consumer app.

## Jobs / opportunities

Prioritized; each maps to ~one downstream vertical slice.

1. **Know & maintain what's on each spool** — record spools, update remaining
   weight fast (weigh directly, or "I used X g"), see remaining in g and m.
   *(slice: `spools`)*
2. **Reference material properties** — density, drying params, humidity
   sensitivity as the single source of truth feeding length & humidity status.
   *(slice: `materials`, seeded at boot)*
3. **See stock at a glance** — value, remaining weight, split by material,
   soon-empty spools. *(slice: `dashboard`)*
4. **Catch damp sensitive filament before it prints** — ingest SHT31 readings
   per drybox over MQTT, colour status by the stored material's sensitivity
   threshold. *(slices: `locations` + `humidity` — the differentiator)*
5. **See the farm's live state at a glance** *(added 2026-07-22)* — know from
   the open Filature tab whether each machine is idle, printing (progress,
   job, temperatures) or unreachable, without opening PrusaLink, Fluidd or
   Bambu Studio. *(slices: `22a-machine-link-rest` + `22b-machine-link-bambu`;
   follow-up noted: sync loaded spools from machines that report them —
   Bambu AMS first, slice `23`.)*

## Desired outcomes & success measures

Observable, personal-scale (not vanity totals):

- **Trusted over guessing.** The user consults Filature instead of eyeballing
  spools or reaching for Spoolman — measure: it stays the open tab; stock
  entries stay current (spools get weight updates, not abandoned).
- **A wet-print failure is prevented at least once.** The humidity panel flags a
  sensitive drybox over threshold and the user dries before printing — the
  concrete signal the differentiator earned its build.
- **Quoting uses real material cost** (later) — a part quote pulls €/g from
  actual spool data rather than a guess.

## Riskiest assumptions

Worst first. **Scope note (2026-07-05):** humidity/MQTT is deferred out of
v0-v1 — no physical sensors on hand yet. So the humidity assumption, once #1,
can't be tested now and isn't the near-term risk. Build the stock platform
first; revisit the humidity differentiator when sensors exist.

1. **Weigh-to-net data entry is fast enough to stay current.** With humidity
   deferred, this is now the top risk: if logging a spool or a weight update is
   tedious, entries rot and the "trusted over guessing" outcome fails — and
   without the humidity differentiator, a stock-only Filature must justify
   itself against Spoolman on data-entry quality alone.
   **Cheapest test:** the add/edit weigh→tare→net flow and inline weight edit
   must feel like a ~10-second gesture — validate by using it for real spools.
2. **Single self-hosted operator is enough.** No multi-user, no concurrency
   beyond one person. Low risk given scope. (Persistence is PostgreSQL in the
   deployed env — [ADR-0003](../adr/0003-postgresql-persistence.md); the binary
   stays self-sufficient for everything except the database.)
3. *(deferred)* **The humidity differentiator works end-to-end and is
   actionable** (SHT31 → MQTT → per-material threshold status). Untestable until
   sensors exist. A throwaway spike (`rumqttc` subscribe → status vs threshold)
   was built and removed; regenerate it when the humidity slice is picked up.

## Scope

**In (v0-v1 — core stock, the whole near-term product):** material CRUD
(seeded), spool CRUD, fast remaining-weight update, filterable/sortable spool
list (no reload), spool detail, location CRUD (as a plain storage place — no
sensor), dashboard (value, remaining, split by material, soon-empty).
**Internationalised UI** — en + fr shipped, extensible to more locales; no
hardcoded UI strings (see [ADR-0001](../adr/0001-language-and-i18n.md)). Domain
code identifiers are English (see the glossary).

**Deferred (post-v1 — humidity, the eventual differentiator):** MQTT topic on a
location, MQTT subscription task inserting readings, per-drybox humidity panel
with material-sensitivity-coloured status, live refresh. Picked up once physical
sensors exist. The material referential still stores humidity
sensitivity/thresholds now (cheap, and it's the source of truth the humidity
slice will read), but nothing consumes it in v1.

**Out (explicit — the rework firewall):** per-job consumption tracking, per-print
material cost, per-machine history, PDF/CSV export, multi-user/auth, mobile
native app, automated backup tooling (rely on Postgres `pg_dump`/managed backups
for now), any npm/JS build beyond htmx.

## Proposed features (hypotheses)

Traced to opportunities; full UI is specified in
`init_assets/design_handoff_filature/` (6 screens, hi-fi). Each screen is a
hypothesis serving a job above: dashboard→#3, spools list/detail/form→#1,
materials table→#2, humidity panel + locations→#4.

## Open questions

- Exact MQTT topic→location mapping convention (one topic per drybox? payload
  shape of the relay?) — resolve in the humidity slice / prototype.
- Threshold model: fixed per-sensitivity (Low=40 / Medium=30 / High=15 %HR per
  handoff) vs per-material override — start fixed, revisit if too coarse.
- Surface a spool "archived" vs "empty" distinction on the dashboard? Handoff
  implies yes; confirm during `spools`.
