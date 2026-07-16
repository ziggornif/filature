## Agent Brief — 16 Print Heads + updated printer catalogue + Multi-Slot module

> _AI-generated brief (Claude Code, orchestrator). Reviewed before delegation._

**Category:** feature
**Summary:** Introduce **Print Head** as a first-class Printer dimension (N heads, default 1),
refresh the curated Bambu Lab / Prusa model catalogues, and unify `INDX` + `Tool Changer` +
`Multi-colour unit` into a single brand-agnostic **Multi-Slot** Filament Module.

**Slice / context:**
Builds on the `printers` slice (15a aggregate + brand/module config + slot derivation, 15b
spool loading). Today a Printer has a single `Filament Module` (`None | Ams | Mmu | Indx{slots}
| ToolChanger{heads} | MultiColour{slots}`) that alone derives the Slot layout; multi-head is
modelled only as Prusa XL's `Tool Changer`. Two problems this slice fixes: (1) the catalogue is
stale and missing current machines (Bambu X2D/H2S/H2D/H2C/A2L/P2S, Prusa MK3.x/CORE One+); (2)
multi-head is a Prusa-only, module-shaped concept, and `INDX` / `Multi-colour unit` are two
names for the same generic "automatic multi-slot changer". This slice generalises heads and
collapses those module kinds.

**This is Slice A of two.** Slice A ships: catalogue refresh, the `heads` dimension (simple
direct-spool heads), and the `MultiSlot` module. **Slice B (separate, later)** ships the Bambu
topology: N AMS units attachable to one machine and a per-head "simple vs bound-to-an-AMS"
choice with free spool mapping. Nothing in Slice A should assume a head can bind to an AMS.

**New / changed ubiquitous language** (apply to `docs/glossary.md`):
- **Print Head** _(Tête)_ — a physical toolhead on a Printer carrying one filament path. A
  Printer has **N Print Heads, N ≥ 1, default 1**. When N > 1 each head is an independent
  direct-spool Slot. Replaces the `Tool Changer` module as the multi-head concept.
- **Filament Module** — kinds become: **AMS** (Bambu, 4 Slots + 1 external Slot), **MMU**
  (Prusa, 5 Slots), **Multi-Slot** (brand-agnostic automatic multi-material changer with a
  fixed chosen slot count). `INDX`, `Tool Changer` and `Multi-colour unit` are **removed** as
  distinct terms; `INDX` and `Multi-colour unit` become **Multi-Slot**, `Tool Changer` becomes
  the **Print Head** count.

**Desired behavior:**

- **Print Head count.** Every Printer carries a head count (≥ 1, default 1), independent of its
  Filament Module. It is selectable in the form only for models that support > 1 head; for all
  other models it is fixed at 1 and no selector is shown.
  - **Multi-head layout:** when heads > 1 the layout is **N direct-spool head Slots** (group
    "Têtes", keys `head-0 … head-N-1`), and the Filament Module **must be `None`** in Slice A
    (no AMS/MMU/Multi-Slot combined with multiple heads yet — that is Slice B). Prusa XL keeps
    behaving exactly as before (1/2/5 head slots) but now via the head count, not a module.
  - when heads == 1 the layout derives from the Filament Module as today.

- **Updated catalogue (curated per brand, free text for Other):**
  - **Bambu Lab:** `A1 mini`, `A1`, `A2L`, `P1P`, `P1S`, `P2S`, `X1 Carbon`, `X2D`, `H2S`,
    `H2D`, `H2C`. (Removes `X1C`, `X1E`; adds A2L, P2S, X2D, H2S, H2D, H2C. `X1C` → `X1 Carbon`.)
  - **Prusa:** `MINI+`, `MK3 / MK3S / MK3S+`, `MK4S`, `CORE One+`, `CORE One L`, `XL`. (Removes
    bare `MK4`; `CORE One` → `CORE One+`; adds `MK3 / MK3S / MK3S+`.)
  - Curated lists stay hardcoded constants shared by domain + form (no editable referential).
  - **Existing DB rows with retired model strings keep working** (model is free text; Bambu
    module validation matches any model). Prusa module-validation literals must track the
    renames (`CORE One` → `CORE One+`).

- **Head count per model (defaults + allowed set):**
  - Bambu `X2D` → 2 (fixed), `H2D` → 2 (fixed); every other Bambu model → 1.
  - Prusa `XL` → selectable {1, 2, 5}, default 2; every other Prusa model → 1.
  - Other → 1.

- **Filament Module per model (heads == 1 cases):**
  - **Bambu, most models** (`A1 mini`, `A1`, `A2L`, `P1P`, `P1S`, `P2S`, `X1 Carbon`, `H2S`):
    `None` or `AMS` (AMS = 1 external Slot + 4 AMS Slots), as today. Default AMS.
  - **Bambu `H2C`:** `Multi-Slot` with a **fixed 7** slots (the automatic 7-nozzle changer).
    Default Multi-Slot 7. `None` (single direct Slot) also allowed.
  - **Bambu `X2D` / `H2D`:** heads = 2, Module `None` only (Slice A).
  - **Prusa `CORE One+` / `CORE One L`:** `None`, `MMU` (5), or `Multi-Slot` with slots ∈
    {4, 8} (default 4). (This is the former INDX.)
  - **Prusa other single-head** (`MINI+`, `MK3 / MK3S / MK3S+`, `MK4S`): `None` or `MMU` (5).
  - **Other:** `None` (1 Slot) or `Multi-Slot` with slots ∈ {2, 3, 4, 5, 6, 8}, default 4.
    (This is the former Multi-colour unit; the operator may load different materials, not only
    colours — hence the rename.)

- **Slot group labels stay brand-appropriate** in the UI: Prusa Multi-Slot shows "INDX", Bambu
  H2C Multi-Slot shows a "buses" label, Other Multi-Slot shows a generic multi-material label —
  driven by i18n, EN + FR parity. The domain term is Multi-Slot; presentation may vary by brand.

- **Edit re-derivation** keeps the existing merge-by-slot-key contract from 15a/15b: changing
  heads or module re-derives the layout and preserves loaded-Spool assignments for slot keys
  that still exist; vanished slots are unloaded per 15b rules. Unknown id → 404 (no silent no-op).

- **Instance export / import** round-trips the head count and the new module kind, extending the
  printer round-trip added for instance transfer. A printer exported before this slice (no head
  field) imports as heads = 1. A `tool_changer` / `indx` / `multi_colour` kind in an old export
  imports to the new representation (see migration mapping).

- **Data migration** (single new migration): add the head column (default 1); convert existing
  `tool_changer` rows to head count + module `None`; convert existing `indx` and `multi_colour`
  module kinds to `multi_slot` (preserving their slot count). Existing `printer_slots` rows keep
  their stored keys unchanged.

**Key interfaces:** (glossary + domain terms — no file paths)
- `Printer` / `NewPrinter` — gain a validated head-count field (u8, ≥ 1); `Printer` layout
  derivation takes head count into account.
- `Module` — drop the `Indx`, `ToolChanger`, `MultiColour` variants; add `MultiSlot { slots }`
  (storage kind `"multi_slot"`). `kind()`, `count()`, `from_storage()` updated accordingly.
- Module / head validation — validity now depends on (brand, model, heads, module): heads > 1
  ⇒ module `None`; per-model allowed head sets and module sets as specified above. Invalid combos
  → `DomainError::InvalidPrinterConfiguration`.
- Slot-layout derivation — heads > 1 yields N direct head Slots; heads == 1 yields the module
  layout; `MultiSlot { slots }` yields `slots` Slots.
- Curated model constants (`BAMBU_MODELS`, `PRUSA_MODELS`) and head/module option tables shared
  by domain and the printer form's client script.
- Printer web form command/DTO — accepts the head count and the `multi_slot` module kind.
- Instance export/import mapper for printers — carries head count + module kind, with the
  backward-compat defaults above.

**Acceptance criteria:**
- [ ] A Bambu `X2D` and a Bambu `H2D` each derive **2 direct head Slots** (keys `head-0`,
      `head-1`), module `None`, and reject any non-`None` module.
- [ ] A Bambu `H2S` derives its normal 1-head layout (module `None` or AMS as chosen).
- [ ] A Bambu `H2C` derives **7** Multi-Slot Slots by default; `None` gives a single Slot.
- [ ] A Prusa `XL` still derives 1 / 2 / 5 head Slots for the three head counts and rejects
      head counts outside {1, 2, 5}; XL no longer uses a `Tool Changer` module.
- [ ] A Prusa `CORE One+` and `CORE One L` accept Multi-Slot with slots ∈ {4, 8} and reject
      other counts; `MMU` and `None` still valid.
- [ ] An `Other` printer accepts Multi-Slot with slots ∈ {2, 3, 4, 5, 6, 8} and rejects other
      counts.
- [ ] The `Module` enum no longer has `Indx`, `ToolChanger`, or `MultiColour`; `multi_slot`
      round-trips through `from_storage` / `kind` / `count`.
- [ ] The printer form offers the updated Bambu/Prusa model lists; shows a head selector only
      for XL (1/2/5); shows the correct module choices and fixed counts per model (H2C → 7).
- [ ] Migration converts pre-existing `tool_changer` rows to head count + `none`, and `indx` /
      `multi_colour` rows to `multi_slot`, with slot counts preserved; a printer created before
      the migration still loads and renders.
- [ ] Instance export → import round-trips head count and the `multi_slot` kind; an old export
      without a head field imports as heads = 1 and old `tool_changer`/`indx`/`multi_colour`
      kinds import to the new representation.
- [ ] Glossary updated: **Print Head** added; **Multi-Slot** replaces INDX / Tool Changer /
      Multi-colour unit; Printer Model catalogue lists refreshed. i18n EN + FR parity for all
      new/renamed labels.
- [ ] Domain purity + existing hexagonal sensors stay green; `cargo test` (domain + app
      integration) passes, including updated/added tests for the cases above.

**Out of scope:**
- **Slice B** entirely: N AMS units attachable to one machine, a per-head "simple vs bound-to-an-
  AMS" choice, and free spool→AMS-slot mapping. Do **not** add multi-AMS or head↔AMS binding here.
- No AMS/MMU/Multi-Slot **combined with** multiple heads in this slice (heads > 1 ⇒ module `None`).
- No editable/user-managed model referential — catalogues stay hardcoded constants.
- No change to spool loading rules (exclusivity, auto-unload) beyond the existing merge-by-key
  re-derivation contract.
- No humidity / sensor work (deferred post-v1).

**References:**
- Prior slices: `docs/specs/15a-printers-core.md`, `docs/specs/15b-printer-spool-loading.md`
- Instance transfer printer round-trip: `docs/specs/12-instance-export-import.md`
- Glossary: `docs/glossary.md` ("Printers & filament loading")
- Design / UI: `docs/design.md`, `init_assets/design_handoff_filature/Filature.dc.html`
- ADRs: `docs/adr/` (0001 EN code/i18n UI, 0002 workspace, 0003 Postgres+testcontainers)
