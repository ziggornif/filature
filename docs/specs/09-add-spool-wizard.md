> _AI-generated brief (Claude Code), reviewed before delegation. Tracks GitHub issue #26._

## Agent Brief

**Category:** feature
**Summary:** Replace the single-page add-spool form with a 2-step wizard (state → details), net-weight presets, and a reworked colour picker.

**Slice / context:** Add-Spool use-case slice, superseding the current add form. Today a Spool is created through one flat form that (a) forces status **Sealed** regardless of the entered weight, (b) offers no way to say the reel is already opened or is a refill, (c) takes an unstructured free-typed net weight, and (d) has a poor colour picker whose default black swatch reads as a deliberate "black" choice. This slice is the source of the shared add/edit form that slice `11-edit-spool-form` (#33) reuses.

The hi-fi design is authoritative in `init_assets/design_handoff_filature/Filature.dc.html` (README §5 + §Design Tokens). Net-weight-only entry is a settled decision — see [ADR-0004](../adr/0004-net-weight-no-tare.md); the weigh→tare→net gesture is **removed**, not reintroduced.

**Desired behavior:**
- **Step 1 — Spool condition.** The user first picks one of three conditions:
  - *Neuve* (new): unopened, full ⇒ Remaining Weight = Net Weight, **Spool Status = Sealed**.
  - *Entamée* (opened): already used ⇒ the user enters the current Remaining Weight, **Spool Status = Open**.
  - *Recharge* (refill): a refill without a holder; treated as new in v1 ⇒ **Spool Status = Sealed**. This records a new **Spool Type = Recharge** (vs the default *Complete*).
- **Step 2 — Details.** Shows the chosen condition as a badge with a "Changer" affordance that returns to step 1. Fields:
  - Material (select over the Material referential), Manufacturer (select over the Manufacturer referential + an "Autre…" escape that lets a new brand be named).
  - **Colour** via the reworked picker (see below).
  - **Diameter** as a segmented control. *(Open question — see Out of scope: whether the set is {1.75, 2.85} or adds 3.0.)*
  - **Net Weight** chosen from presets `250 / 500 / 750 / 900 / 1000 (default) / 2000 / 3000 / 5000 g`, plus "Autre…" revealing a free numeric entry. Entered directly per ADR-0004.
  - **Remaining Weight** field is shown **only** when condition = *Entamée*; hidden otherwise.
  - Storage location, Price Paid, purchase/opened dates, notes as today.
- **Colour picker.** A grid of preset swatches, each with a visible label (including *Transparent*); selecting one rings it. Below a "ou personnalisée" separator: a preview chip that is itself a hidden native colour input (pencil badge overlay), a hex text field (`#RRGGBB`, normalised on blur — accepts `#RGB` and adds a missing `#`), the **derived** colour name shown beneath, an inline error when the hex is invalid, and a clear (✕) action. When no colour is chosen, the chip renders a distinct **"no colour" state** (white with a red diagonal slash) so an unset colour is never mistaken for black.
- **Colour name is derived from the hex, not typed:** the preset's label if the hex matches one, otherwise the upper-cased hex (`transparent` ⇒ "Transparent"). The derived value is what gets stored.
- **On save:** `Remaining Weight = (Entamée ? min(Net, entered) : Net)`; `Status = (Entamée ? Open : Sealed)`; then open the new Spool's detail.
- Reachable from the Bobines screen and from the dashboard "＋ Ajouter une bobine" button (that button is wired in slice `10`/#34).

**Key interfaces:** (glossary + API/SPI terms — locate, don't assume paths)
- `Spool` — gains a **Spool Type** attribute (`Complete` | `Recharge`). New glossary term + persistence column/migration.
- `Diameter` — the enum of standard diameters; extend **only if** the 3.0 decision lands (see Out of scope).
- `Colour` — hex with a name that is now **derived**, not free-typed (glossary currently says "optional free-text name" — update it to match).
- `SpoolStatus` — unchanged set (Sealed/Open/Empty/Archived); the initial value is derived from the chosen condition, never hard-forced to Sealed.
- The add-spool **API port** use case — input now carries condition, spool type, optional colour, net weight (preset or custom), and (conditionally) remaining weight; it derives status + remaining.

**Acceptance criteria:**
- [ ] Step 1 offers Neuve / Entamée / Recharge; layout is 3 columns on wide screens, 1 column ≤760px.
- [ ] Step 2 shows the condition badge and a working "Changer" back to step 1.
- [ ] Remaining Weight input appears only for Entamée.
- [ ] Net Weight presets work; 1000 g is the default; "Autre…" allows a free value.
- [ ] A Recharge spool persists Spool Type = Recharge; others = Complete.
- [ ] Initial Spool Status matches the condition (an Entamée spool is Open, not Sealed).
- [ ] Colour picker shows the "no colour" red-slash state when unset, labelled presets, custom hex with blur normalisation, derived name, clear action, and inline error on invalid hex.
- [ ] Saving computes Remaining Weight and Status per the rules above and opens the spool detail.
- [ ] All new UI strings are in the fr + en catalogs; htmx fragments render in the active locale; a render test covers a non-default locale.

**Out of scope:**
- Editing an existing Spool through this form — that is slice `11`/#33 (shares these components).
- Wiring the dashboard "＋ Ajouter une bobine" button — slice `10`/#34.
- Any tare / weighing gesture (removed per ADR-0004).
- **Diameter 3.0 is unresolved:** the glossary defines exactly two standards (1.75 / 2.85). Do **not** silently add 3.0 — the diameter set is settled with the PO before this slice extends the `Diameter` enum. If confirmed, extending the enum + its parse/label + a persistence check is in scope; if not, keep the two-value control.
- NFC/QR tag autofill (separate future opportunity).

**References:**
- Issue: #26 · downstream: #33 (shared edit form), #34 (dashboard add button)
- Design handoff: `init_assets/design_handoff_filature/` (`Filature.dc.html`, README §5 + §Design Tokens)
- Design decisions: `docs/design.md`
- ADRs: `docs/adr/0004-net-weight-no-tare.md`, `docs/adr/0001-language-and-i18n.md`
- Glossary: `docs/glossary.md` (Spool, Net Weight, Remaining Weight, Colour, Diameter, Spool Status)
