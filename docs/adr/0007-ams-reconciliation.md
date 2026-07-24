# ADR-0007 — AMS spool sync is operator-confirmed, keyed by memorized RFID, weight non-authoritative

Status: accepted (2026-07-24)

## Context / forces

Slice `23` lets Filature read a Bambu AMS's live Trays (via the on-demand MQTT
proxy from `22b`) and reconcile them against the operator's Spools, so the AMS's
physical contents no longer have to be re-entered by hand into printer Slots
(`15b`). The captured AMS payload gives, per Tray: filament `tray_type`,
`tray_color`, `tray_sub_brands`, a coarse `remain` percentage, and `tag_uid`
(RFID).

Forces:
- **Trust of machine data varies.** Genuine Bambu spools carry a stable, unique
  RFID `tag_uid`; third-party spools report `0000…` (absent). A single parc is
  mostly third-party, so a matcher that only trusts RFID would be near-useless,
  while one that trusts colour/type blindly will mis-match two identical spools.
- **Filature already owns an authoritative weight.** Remaining weight is a
  precise **weighed net weight** (ADR-0004). The AMS `remain` is a coarse
  estimate, unreliable-to-absent for third-party spools. Letting the machine
  overwrite the weighed value would destroy the more trustworthy number.
- **Loading is a controlled, exclusive assignment.** `15b` makes a Spool loadable
  into at most one Slot across the parc, Sealed/Open only. A sync must not smuggle
  around those invariants.
- **No stable machine→Spool identity exists yet.** Nothing on a Spool records
  which physical filament roll it is, so a first sync has nothing certain to match
  on for third-party spools.

## Decision

1. **Suggest, never silently apply.** Reconciliation computes a suggested Spool
   per Tray and presents them for the operator to confirm or correct. It never
   auto-creates a Spool and never loads a Slot without confirmation.
2. **Match key: RFID first, attributes as fallback.** If a Tray's `tag_uid` is
   non-null and already memorized on a Spool → certain match. Otherwise fall back
   to `tray_type` + colour (+ sub-brand) among **loadable** Spools (Sealed/Open,
   not loaded elsewhere), presented as a best-effort suggestion.
3. **Memorize the RFID UID on the Spool at first confirmation.** Confirming a
   match stores the Tray's `tag_uid` on the matched Spool (new nullable Spool
   attribute), turning every later sync of that roll into a certain match. `0000…`
   is treated as absent and never stored.
4. **Confirmation loads through the existing `15b` use case.** The confirm step
   calls `load_slot` on the AMS-Unit Slot, inheriting exclusivity, loadable-status
   and auto-unload unchanged. The sync widens *how* a Slot gets filled, not the
   loading rules.
5. **Filature weight stays authoritative.** A sync never silently overwrites
   remaining weight. A discrepancy between AMS `remain` and Filature's weighed
   weight is **surfaced** for the operator to resolve deliberately; the alignment
   UI is a design-phase concern.

## Rejected alternatives

- **Auto-load on match.** Zero-click, but a wrong attribute match silently loads
  the wrong Spool into a Slot — worst for the third-party-heavy common case.
- **Auto-create a Spool for an unmatched Tray.** Pollutes the stock with
  duplicates and invents purchase/weight data the machine cannot know.
- **RFID-strict matching.** Certain, but matches nothing for third-party spools —
  useless for the actual parc.
- **Overwrite weight from AMS `remain`.** Destroys the weighed value (ADR-0004)
  and is wrong for third-party spools that report 0/garbage.

## Consequences

- Spools gain a nullable **AMS Tag UID** attribute; export/import round-trips it.
- Bambu only — Prusa/Klipper Machine Links expose no per-Tray filament data, so
  the feature is inert for them.
- First sync of a third-party roll is attribute-suggested (operator confirms);
  it becomes certain only if/when a genuine RFID UID is ever memorized — which for
  a pure third-party spool never happens, and that is acceptable.
- The weight-discrepancy resolution UI is deferred to the design phase (open
  question from discovery); until designed, a sync shows the discrepancy read-only
  and changes no weight.
