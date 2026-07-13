> _AI-generated brief (Claude Code), reviewed before delegation. Tracks GitHub issue #33._

## Agent Brief

**Category:** feature (fixes two defects)
**Summary:** Rework the "Modifier la bobine" form to reuse the wizard's shared details step, making Colour and Status editable.

**Slice / context:** Edit-Spool slice. Two defects today: the edit form does **not** let the user change the Spool's Colour, and it does **not** let the user change the Spool Status. The design uses a **single shared add/edit form** — the edit path should reuse the wizard's step-2 details component (built in slice `09`/#26) rather than maintain a separate form. Depends on `09` being in place.

**Desired behavior:**
- Opening "Modifier" on a Spool presents the shared details form, prefilled from the Spool.
- **Colour is editable** through the same reworked picker (labelled presets, custom hex with blur normalisation, derived name, clear, "no colour" state).
- **Status is editable** via the condition model: the form shows the condition badge, and "Changer" lets the user switch condition, which drives Spool Status. Initial condition is derived from current status — **Open ⇒ Entamée**, otherwise **Neuve**.
- Prefill is faithful: Material, Manufacturer, hex Colour, Diameter, Net Weight, Remaining Weight (for an opened spool), storage, price, dates, notes. If the stored Net Weight is not one of the presets, the preset selector shows "Autre…" prefilled with that value.
- Saving applies the same rules as add: `Remaining Weight = (Entamée ? min(Net, entered) : Net)`; `Status = (Entamée ? Open : Sealed)`; then return to the Spool detail.
- No duplicated form logic: the details step is one shared component used by both add and edit.

**Key interfaces:** (glossary + API/SPI terms)
- The **update-Spool API port** use case — accepts the same structured input as add (condition, colour, net weight, conditional remaining) and applies the derive rules.
- `Colour`, `SpoolStatus`, `Spool Type`, `Net Weight`, `Remaining Weight` — as defined for slice `09`; this slice consumes them, it does not redefine them.

**Acceptance criteria:**
- [ ] From "Modifier", the Colour can be changed with the full picker and persists.
- [ ] From "Modifier", the Status can be changed via the condition (badge + "Changer") and persists.
- [ ] The edit form is the shared wizard details component — no separate/duplicated form.
- [ ] Prefill is correct, including custom Net Weight and Remaining Weight for an opened spool.
- [ ] Save re-derives Remaining Weight and Status correctly and returns to detail.
- [ ] Strings in fr + en; a render test covers a non-default locale.

**Out of scope:**
- Building the shared components themselves (that is slice `09`/#26).
- Adding new fields beyond what add supports.
- The step-1 condition-choice screen as an add entry point (edit opens straight to details with the condition derived).

**References:**
- Issue: #33 · depends on: #26 (shared form + colour picker)
- Design handoff: `init_assets/design_handoff_filature/Filature.dc.html` (shared add/edit form)
- Sibling brief: `docs/specs/09-add-spool-wizard.md`
- Glossary: `docs/glossary.md` (Spool, Colour, Spool Status, Net Weight, Remaining Weight)
