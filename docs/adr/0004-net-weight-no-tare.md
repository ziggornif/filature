# ADR-0004 — Net Weight entered directly; drop the Tare / weigh-total flow

Status: accepted (2026-07-06)

## Context / forces

The source brief and the original glossary modelled spool weighing as a
**weigh → tare → net** gesture: the operator weighs the whole spool (reel +
filament), subtracts the empty-reel **Tare**, and derives **Net Weight** (filament
alone). Riskiest-assumption #1 (`docs/product/brief.md`) is that data entry is
fast enough to stay current; the weigh-total flow was meant to serve it.

On brainstorming the `spools` slice the product owner rejected the tare model:
in practice the operator does not know a reel's empty weight, so a
weigh-total-minus-tare gesture is *not* actually fast — it blocks on a number the
user cannot readily produce. Meanwhile the number the user *does* have is the
manufacturer's advertised filament weight, printed on every spool label
("1 kg" = 1000 g net).

This reverses a modelled decision (Tare was a named glossary term), so it is
recorded here.

## Decision

**Net Weight is entered directly** (from the spool label), and **Tare is
dropped** from the model entirely.

- `Spool` stores `net_weight` (filament weight, entered) and `remaining_weight`;
  no tare field, no whole-spool total is ever stored.
- On add, `remaining_weight = net_weight` (a Sealed spool is full).
- Remaining-weight updates (the `spools` operations slice, 03b) are done by
  **direct entry** — "remaining is now X g" — or by **consumption** — "used Y g"
  (`remaining -= Y`). Weighing the whole spool to derive remaining is dropped,
  because it too would require the unknown tare.
- **Net Weight** is redefined in the glossary as "the filament weight recorded
  for a spool (from the label)"; **Tare** is removed as a domain term.

## Rejected alternatives

- **Keep Tare, compute net = total − tare.** Matches a scale-first workflow and
  the original brief. Rejected: the operator does not know the tare, so the
  gesture is slow in exactly the way assumption #1 warns against.
- **Optional tare (simple by default, tare for those who want it).** Rejected as
  YAGNI: adds a second weighing model and branch for a capability the single
  operator has said he won't use.

## Consequences

- `docs/glossary.md`: **Tare** removed; **Net Weight**, **Remaining Weight**,
  **Remaining Ratio**, **Stock Value** re-worded to not reference tare or a
  weighed total. **Remaining Length** unchanged (still derived from Material
  density + spool diameter).
- `docs/product/brief.md`: the "weigh directly / weigh→tare→net" phrasing in
  opportunity #1 and riskiest-assumption #1 is superseded by this ADR; the fast-
  entry outcome is now served by direct net entry + "used X g".
- The `spools` operations slice (03b) `adjust-weight` use case has two inputs
  (set-remaining, consume), not a weigh-total input.
- No change to Postgres/hexagonal decisions.
