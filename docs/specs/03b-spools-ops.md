## Agent Brief — 03b Spools (ops: adjust-weight, status transitions, archive/restore, Stock Value)

**Category:** feature
**Summary:** Give a Spool its operational lifecycle — record consumption / set remaining weight, drive Spool Status through Sealed→Open→Empty, archive and restore a spool, and surface Stock Value on the list — plus finish the Money invariant.

**Slice / context:**
Second increment of the `spools` slice. 03a (core) already ships the `Spool`
aggregate, `NewSpool`, `Colour`/`Diameter`, the `spools` API port
(`add`/`edit`/`list`/`view`), the `SpoolRepository` SPI (`insert`/`update`/`list`/`get`),
the read model joined by the adapter (carries Material `name` + `density`), the
htmx list (filter/sort) and detail view, and the derived Remaining Ratio /
Remaining Length. In 03a a spool is created with `remaining = net`, status
**Sealed**, and nothing ever changes its Remaining Weight or Status after
creation. `edit` clamps Remaining down if Net is lowered but does not otherwise
touch Remaining or Status. This increment adds the operational surface that 03a
explicitly deferred. Weight modelling still follows **ADR-0004** (Net Weight
entered directly; no Tare).

**Desired behavior:**

- **Set remaining weight.** Operator records "remaining is now X g" for a spool.
  X must be within `0..=net`; `X > net` is rejected as a validation error (422 at
  the form, not a 500). The resulting Status is derived from the new remaining
  (see *Status derivation*).
- **Record consumption.** Operator records "used Y g" for a spool. `Y` is
  subtracted from Remaining with a floor at 0 (a consumption larger than what
  remains empties the spool rather than going negative). Status derived from the
  result.
- **Status derivation (pure, from remaining vs net).** After any weight change:
  `remaining == 0` ⇒ **Empty**; `remaining == net` ⇒ **Sealed**; otherwise
  ⇒ **Open**. This is the only source of Sealed/Open/Empty — there is no separate
  "open" action. **Archived is never produced by weight** (it is the explicit flag
  below) and weight operations on an Archived spool are rejected.
- **Archive a spool.** Any non-archived spool (Sealed, Open, or Empty) can be
  archived; it leaves active stock but is kept for history. Archiving an already
  Archived spool is rejected. Archived spools are excluded from the default list.
- **Restore a spool.** An Archived spool can be returned to active stock; its
  Status is re-derived from its (unchanged) Remaining vs Net using the same
  derivation rule. Restoring a non-Archived spool is rejected.
- **Stock Value.** The list surface shows Stock Value: the sum over the
  **non-archived** spools currently matching the active list filter of
  `(Remaining Weight ÷ Net Weight) × Price Paid`. It respects the active
  Material/Status filter and recomputes on the same htmx swap that updates the
  table. Archived spools never contribute.
- **Finish the Money invariant.** `Money` becomes a validated value object that
  cannot represent a negative amount; construction of a negative Money fails with
  a domain error. The Price Paid form maps that domain error to a 422 (replacing
  the ad-hoc handler-level negative check added at the end of 03a). Existing
  callers (seed, adapter, stub, `NewSpool`) go through the validated constructor.
- **Not-found handling.** Every operation that targets an existing spool by id
  (`set remaining`, `consume`, `archive`, `restore`) returns a distinct
  not-found outcome for an unknown id, surfaced as **404** at the web layer — the
  SPI must report a 0-row update as not-found rather than a silent success (do not
  reintroduce the TD-003 silent-no-op class in this slice).
- **i18n:** all new UI strings (form labels, buttons, status/error messages) via
  the en/fr catalogs (ADR-0001); new keys added to **both** locales; no hardcoded
  strings. Status badge labels reuse the existing Spool Status keys.

**Key interfaces:** (glossary + API/SPI terms — no file paths)
- `Money` (`shared/`) — promoted to a validated value object: a constructor that
  rejects a negative amount with a `DomainError` (e.g. `NegativeMoney`). No public
  path that yields a negative Money.
- `Spool` — gains behavior for its lifecycle, as domain methods returning a
  `Result` with a `DomainError` on an illegal transition:
  - set-remaining (reject `> net`), derive Status;
  - consume (floor at 0), derive Status;
  - archive (reject if already Archived);
  - restore (reject if not Archived), derive Status from remaining vs net.
  A single pure Status-derivation helper (`remaining`, `net` ⇒ `SpoolStatus`)
  backs the weight operations; `Archived` is outside it.
- `SpoolStatus` — all four states now reachable: `Open`, `Empty` via weight;
  `Archived`/back via archive/restore; `Sealed` on full remaining.
- `spools` **API port** — new use cases: set-remaining `(SpoolId, Grams)`,
  consume `(SpoolId, Grams)`, archive `(SpoolId)`, restore `(SpoolId)`, each
  `Result` with a not-found and a validation error path; and
  `stock_value(filter) -> Money`.
- `SpoolRepository` **(SPI)** — reuse `update` for the mutated aggregate
  (load → mutate in the use case → persist); add/confirm a not-found signal so a
  0-row update maps to a not-found error, not a false success. Add a stock-value
  aggregate capability computed in the adapter (SQL sum over non-archived matching
  the filter) — do not load all rows into the domain to sum them.
- Reuse the existing `spools` read model and the shared `Grams`/`Money` value
  objects; the `spools` slice must **not** `use` the `materials` slice (unchanged
  cross-slice rule — Material fields arrive as read-model primitives).

**Acceptance criteria:**
- [ ] `Money` cannot be constructed negative; a negative Price Paid is rejected via
      the domain error and surfaced as 422; no ad-hoc handler-level negative check
      remains.
- [ ] Set-remaining to a value in `0..=net` updates Remaining and derives Status
      (`0`⇒Empty, `net`⇒Sealed, else⇒Open); set-remaining `> net` is rejected 422.
- [ ] Consume subtracts with a floor at 0 (over-consumption ⇒ Remaining 0 ⇒ Empty);
      Status derived from the result.
- [ ] Weight operations (set-remaining, consume) on an Archived spool are rejected.
- [ ] Archive moves any non-archived spool to Archived; archiving an Archived spool
      is rejected; archived spools are absent from the default list.
- [ ] Restore returns an Archived spool to active stock with Status re-derived from
      remaining vs net; restoring a non-Archived spool is rejected.
- [ ] The list can be filtered to show Archived spools via an explicit status
      filter.
- [ ] Stock Value equals `Σ (remaining÷net × price_paid)` over the non-archived
      spools matching the active filter, excludes Archived, and updates on the htmx
      list swap (unit-tested with known values; adapter-tested as a SQL aggregate).
- [ ] Every id-targeted operation returns not-found for an unknown id and surfaces
      404 at the web layer; the SPI treats a 0-row update as not-found (no silent
      no-op).
- [ ] Detail page exposes set-remaining and consume forms (htmx, fragment swap
      updating status badge + Ratio + Length) and an Archive button that becomes a
      Restore button on an archived spool.
- [ ] All new UI strings exist in both en and fr; none hardcoded; status badges
      reuse existing Spool Status keys.
- [ ] Tests at every layer: domain (status-derivation / transition table across
      each status × each op, `>net` reject, consume floor, archive/restore guards,
      negative-Money reject), use cases via SPI stubs, SPI adapter integration
      (Testcontainers — status/remaining persistence, stock-value aggregate,
      not-found), e2e (add → consume → Empty → archive → restore). Offline `.sqlx`
      metadata regenerated so CI builds with `SQLX_OFFLINE`.

**Out of scope:**
- Dashboard aggregates beyond the single list Stock Value stat (separate slice).
- Any **Location** reference on a spool (the `locations` slice does not exist yet).
- Delete of a spool (retirement is Archive; there is no hard delete).
- Any tare / weigh-total input (dropped, ADR-0004).
- A weight-change history/audit log (only current Remaining is stored).
- The TD-003 fix in the **materials** slice (this brief only prevents the same
  class in `spools`; materials is a separate follow-up).

**References:**
- Product brief: `docs/product/brief.md` (opportunity #1, riskiest assumption #1)
- Prior increment: `docs/specs/03a-spools-core.md`
- Architecture (slices, API/SPI, cross-slice read-model rule): `docs/architecture.md`
- Design (spool list table+card, inline weight, detail): `docs/design.md`
- ADRs: `docs/adr/0001-language-and-i18n.md`, `docs/adr/0004-net-weight-no-tare.md`
- Glossary: `docs/glossary.md` (Spool, Net/Remaining Weight, Remaining Ratio,
  Stock Value, Price Paid, Spool Status)
- Tech debt: `docs/tech-debt.md` (TD-003 class avoided here; TD-005/006/007/008
  remain open, not addressed by this brief)
- Pattern to mirror: the `spools` core increment (03a) and the `materials` slice —
  model/ports/usecases/stubs layout, htmx fragment swaps, SQLx adapter with
  offline metadata.
