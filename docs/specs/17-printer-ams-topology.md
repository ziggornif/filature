## Agent Brief — 17 Bambu multi-AMS topology + per-head feed + free spool mapping

> _AI-generated brief (Claude Code, orchestrator). To be reviewed before delegation._

**Category:** feature
**Summary:** Let a Printer carry **N AMS Units**, give each **Print Head** a **feed mode**
(Direct spool vs AMS-fed), and let the operator map Spools freely into the resulting Slots —
including the combination Slice A forbids (multiple heads **plus** an AMS).

**Slice / context:**
This is **Slice B**, the second half of the printer topology rework begun in
`docs/specs/16-printer-heads.md` (Slice A). Slice A introduced the **Print Head** dimension
(N heads, default 1, each a direct-spool Slot) and unified `INDX`/`Tool Changer`/`Multi-colour`
into the **Multi-Slot** module, but deliberately deferred the Bambu topology: it forces
`module = None` whenever heads > 1, and a Printer has at most one AMS. This slice lifts both
limits. Real Bambu setups connect **several AMS units** to one machine (AMS Hub), and a
multi-head machine (H2D) can have some heads fed from an AMS and others running a direct spool.

**New / changed ubiquitous language** (apply to `docs/glossary.md`):
- **AMS Unit** _(Unité AMS)_ — one physical AMS attached to a Bambu Printer, providing **4
  Slots**. A Printer has **0..N AMS Units**. Replaces the previous notion that AMS is a single
  boolean module on the Printer. Bambu only.
- **Feed Mode** _(Mode d'alimentation)_ — a per-**Print Head** setting: **Direct** (the head runs
  a single directly-mounted Spool via its own Slot) or **AMS-fed** (the head draws from the
  Printer's AMS Unit pool and has no direct Slot of its own).
- **Filament Module** — the `AMS` kind is retired as a Printer-level module for Bambu; AMS
  becomes a **set of AMS Units** on the Printer instead. `MMU` and `Multi-Slot` (Prusa / Other)
  are unchanged by this slice.

**Desired behavior:**

- **AMS Units on a machine (Bambu only).** The operator can add and remove AMS Units on a Bambu
  Printer, from **0 up to a capped N** (propose **4**, matching AMS Hub — confirm the cap). Each
  AMS Unit contributes **4 Slots** (keys `ams{u}-0 … ams{u}-3`, `u` = unit index). Prusa and
  Other Printers have no AMS Units. Removing an AMS Unit unloads any Spools in its Slots
  (per the 15b auto-unload contract).

- **Per-head Feed Mode.** Each of the Printer's Print Heads is independently **Direct** or
  **AMS-fed**.
  - A **Direct** head contributes one direct Slot (key `head-N`), as in Slice A.
  - An **AMS-fed** head contributes **no Slot of its own**; it consumes from the Printer's shared
    AMS Unit pool. An AMS-fed head is only valid when the Printer has ≥ 1 AMS Unit.
  - Single-head Bambu (e.g. `P1S`, `X1 Carbon`) with its head AMS-fed + N AMS Units reproduces
    the classic AMS setup. `H2D` (2 heads) may mix, e.g. head-0 AMS-fed, head-1 Direct.

- **Combined layouts now allowed.** heads > 1 **with** AMS Units is valid (the Slice-A
  restriction `heads > 1 ⇒ module None` is removed for Bambu). The full Slot layout of a Bambu
  Printer is: one direct Slot per Direct head, plus 4 Slots per AMS Unit. At least one Slot must
  result (reject a config with zero heads and zero AMS, or all-AMS-fed heads and zero AMS Units).

- **Free Spool mapping.** The operator maps Spools into any resulting Slot — direct head Slots
  and AMS Unit Slots alike — reusing the existing loading UX and rules from 15b: a Spool is
  loaded into **at most one Slot across all Printers** (exclusivity), only **Sealed**/**Open**
  Spools load, **Empty**/**Archived** auto-unload. No change to those rules; this slice only
  widens the set of Slots a Spool can be mapped into.

- **Edit re-derivation** keeps the merge-by-slot-key contract from 15a/15b: changing AMS Unit
  count or a head's Feed Mode re-derives the layout and preserves loaded-Spool assignments for
  Slot keys that still exist; vanished Slots are unloaded. Unknown id → 404.

- **Prusa / Other unaffected.** Prusa (heads via Slice A, MMU / Multi-Slot modules) and Other
  (Multi-Slot) behave exactly as after Slice A. Feed Mode and AMS Units are Bambu-only concepts.

- **Instance export / import** round-trips the AMS Unit set and per-head Feed Mode. An export
  produced after Slice A (single-AMS-or-none, no per-head mode) imports forward: a Slice-A Bambu
  with the old AMS module becomes **1 AMS Unit + its single head AMS-fed**; a Slice-A Bambu with
  module None becomes **0 AMS Units + head Direct**.

- **Data migration** (single new migration): introduce persistence for the AMS Unit set and the
  per-head Feed Mode; migrate existing Bambu Printers — an AMS-module Printer → 1 AMS Unit with
  its head AMS-fed; a None-module Bambu → head Direct, 0 AMS Units. Existing `printer_slots` rows
  keep their stored keys where the key still exists under the new derivation.

**Key interfaces:** (glossary + domain terms — no file paths)
- `Printer` / `NewPrinter` — gain an ordered set of **AMS Units** (Bambu only) and a per–Print
  Head **Feed Mode**; head count still from Slice A.
- Layout derivation — takes (brand, model, heads, per-head Feed Mode, AMS Unit count) and yields:
  a direct Slot per Direct head + 4 Slots per AMS Unit. Validity: AMS-fed head ⇒ ≥ 1 AMS Unit;
  ≥ 1 Slot total; AMS Units only on Bambu. Invalid → `DomainError::InvalidPrinterConfiguration`.
- `Module` — the Bambu `Ams` module kind is removed in favour of the AMS Unit set; `MMU` /
  `MultiSlot` untouched. `kind()` / `from_storage()` updated.
- Printer web form command/DTO — add/remove AMS Units, set each head's Feed Mode.
- Spool loading port — unchanged contract; operates over the widened Slot set.
- Instance export/import mapper for printers — carries AMS Units + per-head Feed Mode with the
  forward-migration defaults above.

**Acceptance criteria:**
- [ ] A single-head Bambu with head **AMS-fed** and **2 AMS Units** derives **8** Slots
      (`ams0-0..3`, `ams1-0..3`) and **no** direct head Slot.
- [ ] A single-head Bambu with head **Direct** and **0 AMS Units** derives exactly **1** direct
      Slot.
- [ ] An `H2D` (2 heads) with head-0 **AMS-fed**, head-1 **Direct**, **1 AMS Unit** derives
      **4** AMS Slots + **1** direct Slot (`head-1`), total 5.
- [ ] An **AMS-fed** head with **0 AMS Units** is rejected as invalid configuration.
- [ ] A config yielding **0 Slots** is rejected.
- [ ] Adding/removing an AMS Unit or flipping a head's Feed Mode re-derives the layout and
      preserves loaded Spools by surviving Slot key; removed Slots unload their Spool.
- [ ] Spool exclusivity + Sealed/Open-only + Empty/Archived auto-unload rules still hold across
      the widened Slot set (a Spool can be loaded into at most one Slot across all Printers).
- [ ] Prusa and Other Printers are unchanged (no AMS Units, no Feed Mode).
- [ ] Migration converts an existing Bambu AMS-module Printer → 1 AMS Unit + head AMS-fed, and a
      None-module Bambu → head Direct + 0 AMS Units; existing loaded Spools survive where the
      Slot key persists.
- [ ] Instance export → import round-trips AMS Units + per-head Feed Mode; a Slice-A export
      imports forward per the defaults above.
- [ ] Glossary updated (**AMS Unit**, **Feed Mode**; AMS no longer a Bambu module). i18n EN + FR
      parity for all new labels. Domain purity + hexagonal sensors green; full test suite passes.

**Out of scope:**
- Per-AMS-Unit variants (AMS Lite / AMS HT / AMS 2 Pro slot counts, drying, humidity) — every
  AMS Unit is a uniform 4-Slot unit in this slice.
- Binding a specific AMS Unit to a specific head (the pool is shared across AMS-fed heads); no
  per-head→per-unit routing.
- Any change to the Slice-A catalogue, the Multi-Slot module, or Prusa/Other behavior.
- Humidity / sensor work (deferred post-v1).

**References:**
- Slice A (prerequisite): `docs/specs/16-printer-heads.md`
- Prior slices: `docs/specs/15a-printers-core.md`, `docs/specs/15b-printer-spool-loading.md`
- Instance transfer: `docs/specs/12-instance-export-import.md`
- Glossary: `docs/glossary.md` ("Printers & filament loading")
- Design / UI: `docs/design.md`, `init_assets/design_handoff_filature/Filature.dc.html`
- ADRs: `docs/adr/`
