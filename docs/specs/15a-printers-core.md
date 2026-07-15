## Agent Brief — 15a Printers (aggregate + brand/module config + card view + Settings tab)

**Category:** feature
**Summary:** Introduce the `printers` slice — a user-managed set of 3D printers, each
with a **Brand** (Bambu Lab / Prusa / Other), a **Model**, and a brand-specific
**Filament Module** configuration that derives a fixed set of **Slots**. Build the
full printer lifecycle (add / edit / delete), the **Imprimantes 3D** nav page rendering
printer cards with their (still-empty) slots, and the **Settings › Imprimantes** tab
listing printers with add/edit/delete. **Loading spools into slots is slice 15b** —
this slice ships the printers and their empty slot structure only.

**Slice / context:**
New aggregate, following the `locations` (04) / `materials` CRUD template
(hexagonal: pure domain aggregate, API/SPI ports, SQLx adapter, htmx driving adapter)
and the established conventions: opaque id promoted to `shared/`, ULID generated in
the persistence adapter, validated name newtype, i18n en+fr parity, PostgreSQL +
testcontainers (ADR-0003), EN code / i18n UI (ADR-0001), 2-crate workspace (ADR-0002).
The **Printer**, **Printer Brand**, **Printer Model**, **Filament Module** and **Slot**
terms are in the glossary ("Printers & filament loading"). Source of truth for the UI
is `init_assets/design_handoff_filature/Filature.dc.html` (Imprimantes 3D page ~L607,
printer form ~L718, Settings › Imprimantes tab ~L551, and the `buildGroupsForPrinter` /
`savePrinter` logic in the embedded script ~L1298).

**Desired behavior:**

- **Create a printer.** Operator adds a Printer with a **name** (required, trimmed,
  non-blank → 422 like `MaterialName`/`LocationName`), a **Brand** (enum: Bambu Lab /
  Prusa / Other), a **Model**, and the brand-specific module configuration below. On
  save, the Printer's **Slot layout is derived** from (brand, model, module) and
  persisted as a fixed ordered set of empty Slots grouped by label.
- **Model selection (curated per brand, free for Other):**
  - **Bambu Lab** — model from a fixed list `P1S, P1P, X1C, X1E, A1, A1 mini`.
  - **Prusa** — model from a fixed list `MK4S, MK4, MINI+, CORE One, CORE One L, XL`.
  - **Other** — model is **free text** (optional; defaults to "Autre" if blank).
  The curated lists are hardcoded constants (no editable referential — see non-goals).
- **Module configuration → Slot layout (the core rule matrix):**
  - **Bambu Lab:** always one **`Bobine externe`** group (1 slot). An **AMS** toggle
    (default on) adds an **`AMS`** group of **4** slots. AMS off ⇒ external slot only.
  - **Prusa, model `XL`:** always multi-material via a **Tool Changer** — a
    **`Têtes`** group with **N** tool-head slots, N chosen 1–5 (default 2). No module
    select for XL.
  - **Prusa, other models:** a **Module** select — `Aucun (bobine unique)` ⇒ one
    **`Bobine`** group (1 slot); `MMU` ⇒ an **`MMU`** group (5 slots); `INDX` ⇒ an
    **`INDX`** group (5 slots). **INDX is offered only for `CORE One` / `CORE One L`**;
    for any other Prusa model the INDX option is not selectable, and switching the
    model away from CORE One/One L while INDX is selected falls back to `Aucun`.
  - **Other:** a **Module multicolore** toggle (default off). Off ⇒ one **`Bobine`**
    group (1 slot). On ⇒ a **`Module multicolore`** group with **N** slots, N chosen
    from `2, 3, 4, 5, 6, 8` (default 4).
- **Edit a printer.** Operator changes name / brand / model / module. Re-deriving the
  layout may add or remove slots; the update **preserves slot assignments by slot key**
  where the key still exists (see `mergeGroups` in the maquette) — assignments are a
  15b concern, but the merge-by-key contract is defined here so 15a's persistence keeps
  stable slot keys. Unknown id on edit → **404** (0-row update ⇒ NotFound; do **not**
  reintroduce the TD-003 silent-no-op class in this new slice).
- **Delete a printer.** Allowed unconditionally in 15a (no loaded spools exist yet;
  15b adds unload-on-delete). Unknown id → **404**. Available both from the printer
  form (an edit view "Supprimer l'imprimante" button) and the Settings tab row.
- **Imprimantes 3D nav page (`GET /printers`).** New sidebar entry "Imprimantes 3D"
  with a **count badge** (like the spools count). Page shows one **card per printer**,
  grid `auto-fill minmax(340px,1fr)`, each card:
  - top border liner coloured by brand — Bambu `#3a9d5c`, Prusa `#e8720c`,
    Other `#6a63d1`;
  - printer name + model + an edit (pencil) affordance opening the printer form;
  - its groups, each labelled, rendering its slots. In 15a every slot renders as an
    **empty placeholder** (the assign `<select>` and filled-slot rendering are 15b).
    Single-slot groups render the wide row style; multi-slot groups render the 158px
    slot tiles that wrap (match the maquette's `single`/`multi` split).
  - Header subtitle "{n} imprimantes" ({loaded spools count} is added in 15b).
- **Settings › Imprimantes tab.** The tab already exists in the maquette (`printerRows`,
  ~L551). List each printer with name, model, and a **slot summary**
  (`"AMS (4), Bobine externe (1)"` — group label + slot count per group), plus a `+
  Ajouter` control and per-row delete. Add/edit open the same printer form; the form
  remembers whether it was opened from the nav page or the settings tab and returns
  there on save/cancel (`from` in the maquette).
- **i18n:** every new UI string (nav entry, page/tab titles, form labels, brand/model/
  module option labels, buttons, empty-slot placeholder text) via en/fr catalogs; new
  keys added to **both** locales; no hardcoded user-facing strings.

**Key interfaces:** (glossary + API/SPI terms — no file paths)
- `PrinterId` (`shared/`) — opaque id newtype promoted to the shared kernel (15b's
  slot table and any cross-slice read reference it, like `LocationId`/`MaterialId`);
  `new(impl Into<String>)`, `as_str()`.
- `PrinterName` — validated name newtype (trim, reject blank → new
  `DomainError::BlankPrinterName`); `new(impl Into<String>) -> Result`, `as_str()`.
- `PrinterBrand` — enum `{ BambuLab, Prusa, Other }` with the liner colour and the
  allowed model/module rules attached (or expressed in the use case / config).
- `Module` — the module configuration, modelled so the (brand, model, module) → slot
  layout derivation is total and testable. Suggested shape: an enum capturing
  `None` (single slot), `Ams`, `Mmu`, `Indx`, `ToolChanger { heads: u8 }`,
  `MultiColour { slots: u8 }`, with a validating constructor enforcing the matrix
  (INDX only for CORE One/One L; heads 1..=5; multi-colour slots ∈ {2,3,4,5,6,8}).
- `Slot { key: String, group_label: String, position: u16 }` (spool loading added in
  15b) and a `Printer { id, name, brand, model, module, slots: Vec<Slot> }` aggregate;
  `NewPrinter { name, brand, model, module }` (id + derived slots assigned on insert).
- `derive_slots(brand, model, module) -> Vec<Slot>` — the pure function implementing the
  matrix (mirror of the maquette's `buildGroupsForPrinter`); heavily unit-tested.
- `PrintersUseCases` (API port): `list`, `add(NewPrinter)`, `edit(Printer)`,
  `delete(PrinterId)`. Plus a read model `PrinterCard` (see below) for the nav page.
- `PrinterRepository` (SPI port): `list() -> Vec<PrinterCard/Printer>`,
  `insert(NewPrinter)`, `update(Printer)` (persists layout, preserving slot keys),
  `delete(PrinterId)`. `RepositoryError` mirrors the locations/materials SPI with at
  least `Backend`, `NotFound(PrinterId)`, `Domain(#[from] DomainError)`.
- `PrinterCard` read model (in the `printers` slice): printer id, name, model, brand
  (for the liner) and its ordered groups→slots. In 15a slots carry no spool; 15b
  extends it with the loaded-spool display primitives via an adapter join.

**Acceptance criteria (the done contract):**
- Domain unit tests: `PrinterName` rejects blank/whitespace and trims; `derive_slots`
  produces the exact group/slot layout for every branch — Bambu ±AMS (1 vs 1+4), Prusa
  `Aucun`/`MMU`/`INDX`/`XL`(N heads), Other single vs multi(N); the module validator
  rejects INDX for a non-CORE-One model, heads outside 1..=5, and multi-colour counts
  outside the allowed set; edit of an unknown id → `NotFound`.
- Slot-key stability: editing a printer that keeps a group preserves that group's slot
  keys (so 15b assignments survive); reducing a count drops the tail keys only.
- SPI integration (testcontainers): insert→list round-trips brand/model/module and the
  derived slots in order; update persists a changed layout and preserves surviving slot
  keys; update of unknown id → `NotFound` (0-row guard); delete removes the printer and
  its slots.
- Web: `GET /printers` renders one card per printer with the brand liner colour, name,
  model, and empty slots (wide row for single-slot groups, tiles for multi); nav entry
  present with a count badge. `POST /printers` blank name → 422. The printer form shows
  the correct conditional controls per brand and hides INDX for non-CORE-One Prusa
  models. `DELETE /printers/{id}` removes it; unknown id → 404. Settings › Imprimantes
  lists printers with the slot summary and add/edit/delete.
- e2e journey: add a Bambu P1S with AMS → card shows `Bobine externe (1)` + `AMS (4)`
  empty slots → edit to AMS off → card shows only the external slot → add a Prusa CORE
  One with INDX (5 slots) and a Prusa XL with 3 heads → add an "Other" printer, model
  free text, multi-colour 6 → delete one printer.
- i18n: en+fr key parity for every new string; no raw i18n keys leak into rendered HTML
  (assert as in the materials/locations tests).
- All existing tests stay green; clippy + offline build clean; `.sqlx/` cache updated
  (`cargo sqlx prepare`) so CI `SQLX_OFFLINE=true` builds.

**Non-goals / out of scope (YAGNI):**
- **Loading spools into slots** — the assign/unassign `<select>`, filled-slot rendering,
  exclusivity, auto-unload, the "{n} bobines chargées" header stat, and clicking a slot
  to open the spool detail are **all slice 15b**. 15a slots are always empty.
- An **editable model referential** (models are hardcoded curated lists per brand; Other
  is free text). No printer-brand referential either — Brand is a fixed enum.
- Printer nozzle/hotend/build-volume specs, print history, filament-usage tracking,
  connectivity (Bambu Cloud / OctoPrint / PrusaLink), per-slot material presets.
- Reordering printers; filtering/searching the printers page.

**Design notes / constraints:**
- **Migration** `0005_printers.sql` (or next free number): `printers (id TEXT PRIMARY
  KEY, name TEXT NOT NULL, brand TEXT NOT NULL, model TEXT NOT NULL, module_kind TEXT
  NOT NULL, module_count INTEGER NULL)`; `printer_slots (id TEXT PRIMARY KEY, printer_id
  TEXT NOT NULL REFERENCES printers(id) ON DELETE CASCADE, group_label TEXT NOT NULL,
  slot_key TEXT NOT NULL, position INTEGER NOT NULL, spool_id TEXT NULL, UNIQUE(printer_id,
  slot_key))`. The `spool_id` column is created here (nullable, unused in 15a) so 15b is
  a pure additive-logic slice with no schema churn on the hot table; `ON DELETE CASCADE`
  drops a printer's slots with it. Slice isolation: **no FK from `printer_slots.spool_id`
  to `spools` in this migration** — 15b decides whether to add the FK + the exclusivity
  unique index (keeping the `printers` migration free of a `spools` dependency, or add
  it in 15b's migration).
- **Slice isolation:** the `printers` slice does not import `spools`/`locations`; the
  `PrinterId` newtype lives in `shared/`. 15b will fill loaded-spool display fields via
  an adapter join, never a domain import.
- **Store the module config, re-derive slots on read is optional** — persisting the
  concrete slots (with keys) is preferred so 15b assignments attach to durable rows and
  survive app restarts; keep `derive_slots` as the single source that builds them on
  insert and on layout-changing edits.
- **Error hygiene:** new 500 arms follow the existing handlers (TD-005 class — raw
  strings to client — is accepted v1 debt; match the existing pattern, don't widen).
- Keep the UI faithful to the maquette (liner colours, group labels in French via i18n,
  the single-vs-multi slot rendering, the pencil-edit affordance).

**Tech-debt touchpoints:** closes the TD-003 *class* for the new `printers` update
(0-row ⇒ NotFound from the start). Opens the door for 15b; note any `printers.*` /
`nav.printers` i18n key set that 15b will extend.
