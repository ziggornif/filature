## Agent Brief — 15b Printer spool loading (Slot ↔ Spool assignment)

**Category:** feature
**Summary:** Let the operator **load** a Spool into a printer **Slot** and **unload** it,
from the Imprimantes 3D card view. Enforce the exclusivity rule — a Spool is loaded into
**at most one Slot across all Printers** — auto-unload a Spool when it becomes Empty or
Archived, render filled slots with the spool chip/gauge, and open the spool detail from a
loaded slot. Builds directly on slice 15a (printers + empty slot structure).

**Slice / context:**
Extends the `printers` slice from 15a with the loading behaviour. 15a already created the
`printer_slots` rows (with a stable `slot_key` and a nullable `spool_id` column) and the
`PrinterCard` read model with empty slots; this slice fills them in. Source of truth for
the UI is `init_assets/design_handoff_filature/Filature.dc.html` — the filled/empty slot
rendering (~L638–L707), the assign `<select>` option filtering, `assignSlot`/`unassignSlot`
(~L1250), and the `usedIds` exclusivity set + `loadedSpoolsCount` (~L1375). Conventions as
15a: hexagonal, PostgreSQL + testcontainers (ADR-0003), i18n en+fr (ADR-0001), slice
isolation (no `printers`→`spools` domain import; display fields joined in the adapter).
The **Loaded Spool / Loading** term is in the glossary.

**Desired behavior:**

- **Load a spool into a slot.** On the Imprimantes 3D page, an **empty** slot shows a
  `<select>` of loadable spools; choosing one loads it (htmx fragment swap, same pattern
  as the spool weight/location ops). The option list contains **only spools that are
  loadable**: status **Sealed or Open** (never Empty/Archived) **and** not already loaded
  in another slot — plus the currently-loaded spool of *this* slot so it can be reselected.
  Blank ⇒ leave/clear the slot.
- **Unload a spool.** A **filled** slot shows an unassign (✕) control that clears the slot
  (spool returns to the loadable pool). Fragment swap; no spool mutation beyond the link.
- **Exclusivity (the hard rule).** A Spool is loaded into **at most one Slot across all
  Printers** — "deux imprimantes ne peuvent pas avoir une bobine en commun", and not two
  slots of the same printer either. Enforced in the **use case** (loading a spool already
  loaded elsewhere either moves it — unloading the old slot in the same transaction — or is
  rejected; **prefer move** to match the maquette's option filtering, and make it explicit).
  Backed by a DB uniqueness guarantee (partial unique index on `printer_slots.spool_id`
  where not null). Never enforced only in the UI.
- **Loadable-status guard.** Loading a spool that is Empty or Archived → rejected as a
  domain error (defensive: such spools are filtered out of the select). Loading into an
  unknown slot / a slot on an unknown printer → **404**. Loading an unknown spool id → a
  not-found outcome (ids come from a rendered select, so defensive) → 404, **not**
  misreported as another entity's error.
- **Auto-unload on Empty / Archived.** When a loaded Spool transitions to **Empty**
  (remaining weight reaches 0 via a weight edit) or **Archived**, it is **automatically
  unloaded** from its slot. Because slice isolation forbids `spools`→`printers` domain
  imports, do this as **edge orchestration in the `app` crate**: the web handlers that
  set a spool to 0 / archive it also call `PrintersUseCases::unload_spool(SpoolId)` (a new
  API method that clears any slot holding that spool, no-op if none). Document this as the
  chosen cross-slice seam; do **not** add a `spools`→`printers` dependency. (A DB trigger
  is an alternative but is rejected for testability/visibility.)
- **Loading is independent of Location & Status.** Loading does not change the Spool's
  storage Location or its Status; a spool can be both "stored in Drybox 1" and "loaded in
  P1S". The only status coupling is the auto-unload above.
- **Render filled slots.** A filled slot shows the spool colour chip, brand · colour,
  material label, remaining %, and the gauge — single-slot groups in the wide row style,
  multi-slot groups in the 158px tiles (per the maquette). Clicking a filled slot opens
  that **spool's detail** page.
- **Header stat.** The Imprimantes 3D header shows "{n} bobines chargées" = the count of
  distinct loaded spools across all printers (the maquette's `loadedSpoolsCount`).
- **Delete-printer unloads.** Deleting a printer (from 15a) frees its loaded spools
  (they return to the loadable pool). With `ON DELETE CASCADE` on `printer_slots` this is
  automatic at the DB; assert it in a test.
- **i18n:** the empty-slot placeholder, the "— vide —" option, the unload tooltip, the
  header "{n} bobines chargées" string, and any load-error message via en/fr catalogs;
  keys in **both** locales; no hardcoded strings.

**Key interfaces:** (glossary + API/SPI terms — no file paths)
- `PrintersUseCases` gains: `load_slot(PrinterId, slot_key, SpoolId) -> Result` (enforces
  exclusivity + loadable-status, moving the spool off any prior slot),
  `unload_slot(PrinterId, slot_key) -> Result`, and `unload_spool(SpoolId) -> Result`
  (clears whatever slot holds it; no-op if none — used by the archive/empty edge orchestration).
- `PrinterRepository` gains: `set_slot_spool(PrinterId, slot_key, Option<SpoolId>)`,
  `clear_spool(SpoolId)`, and a query powering the loadable-spool select (spools that are
  Sealed/Open and unloaded, plus the current slot's spool). The exclusivity move is one
  transactional path (clear the spool's old slot, set the new) so no intermediate state
  violates the unique index.
- `PrinterCard` read model (from 15a) gains per-slot **loaded-spool display primitives**
  filled by an **adapter join** to `spools` (brand, colour hex, material name, remaining %,
  gauge inputs, spool id for the detail link) — carried as primitives so the `printers`
  slice does not import `spools` (same rule as `SpoolListItem.location_name`).
- New `DomainError` arms as needed: `SpoolNotLoadable`/`SlotNotFound` (→ 409/404 mapping
  at the edge), and a not-found for unknown printer/slot.

**Acceptance criteria (the done contract):**
- Domain unit tests: `load_slot` rejects an Empty/Archived spool; loading a spool already
  loaded elsewhere **moves** it (old slot cleared, new slot set); `unload_slot` clears;
  `unload_spool` clears the holding slot and is a no-op when the spool is unloaded.
- SPI integration (testcontainers): the partial unique index makes a second `set_slot_spool`
  of the same spool into a different slot fail (or the move path succeeds cleanly with the
  old slot emptied); the loadable-spool query excludes Empty/Archived and already-loaded
  spools and includes the current slot's spool; deleting a printer cascades its slot rows
  and frees the spools; the `PrinterCard` join shows the loaded spool's brand/colour/material/
  remaining and `None` for empty slots.
- Auto-unload: editing a loaded spool's remaining weight to 0 (→ Empty) clears its slot;
  archiving a loaded spool clears its slot; both via the app-crate edge orchestration with
  no `spools`→`printers` domain import (assert the seam, e.g. the handler wiring / an
  integration test driving the spool op and then reading the printer card).
- Web: an empty slot's select lists only loadable spools; selecting one swaps the fragment
  to the filled rendering and persists; the ✕ unassigns; a filled slot links to the spool
  detail; the header shows the correct "{n} bobines chargées"; loading into an unknown
  printer/slot → 404.
- e2e journey: add a printer (from 15a) → load a Sealed spool into a slot → verify it is
  gone from another printer's select (exclusivity) → set that spool's remaining to 0 →
  its slot is now empty (auto-unload) and the header count dropped → load another spool,
  then delete the printer → the spool is loadable again.
- i18n: en+fr parity for every new string; no raw keys leak into HTML.
- All existing tests stay green; clippy + offline build clean; `.sqlx/` cache updated.

**Non-goals / out of scope (YAGNI):**
- Per-slot material/colour **validation** against the printer or a print job (any spool can
  go in any slot); filament-usage deduction per print; showing humidity/location warnings
  on the printer card.
- Assigning spools from the **spool** side (a "load onto printer" control on the spool
  detail) — loading is done from the printer card in v1.
- Multi-spool-per-slot, slot reordering, or persisting a print/loading history.
- Filtering the printers page or the loadable-spool select beyond the loadable rule.

**Design notes / constraints:**
- **Migration** `0006_printer_slot_spool.sql` (or next free): add the FK
  `printer_slots.spool_id → spools(id)` (deferred from 15a to keep the `printers` migration
  free of a `spools` dependency) with `ON DELETE SET NULL` (deleting a spool frees its
  slot — a spool is normally archived, not hard-deleted, but be safe), and a **partial
  unique index** `UNIQUE (spool_id) WHERE spool_id IS NOT NULL` enforcing exclusivity.
- **Cross-slice seam.** The auto-unload is the one place two slices meet. Keep both domain
  slices isolated and let the `app` crate orchestrate: the archive/empty spool handlers
  call `PrintersUseCases::unload_spool`. Record this decision in `docs/design.md` (or a
  short ADR if it feels load-bearing) so the pattern is reused, not re-litigated.
- **Two-FK adapter mapping.** Once `printer_slots` has FKs to both `printers` and `spools`,
  the SQLx error mapper must disambiguate the violated constraint (like the spools
  material/location FK disambiguation in slice 04) — map the spool FK to the unknown-spool
  outcome, the printer FK to unknown-printer, exclusivity-index violation to a distinct
  "already loaded" error; an unrecognised constraint stays `Backend`. Regression-test it.
- **Slice isolation:** loaded-spool display fields are primitives joined in the adapter;
  `printers` never imports `spools`. `SpoolId`/`PrinterId` live in `shared/`.
- **Transactionality:** the exclusivity move (clear old slot + set new) must be atomic so a
  concurrent read never sees the spool in two slots and the unique index never trips mid-move.
- Follow ADR-0001/0002/0003; keep the UI faithful to the maquette (option filtering, filled
  vs empty rendering, ✕ unload, header count).

**Tech-debt touchpoints:** the cross-slice edge orchestration for auto-unload is a
deliberate seam, not debt — but if the app-crate wiring grows brittle, file a follow-up to
consider a domain event / outbox. Extends the `printers.*` i18n key set from 15a.
