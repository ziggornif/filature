## Agent Brief — 04 Locations (storage places + Spool→Location assignment)

**Category:** feature
**Summary:** Introduce the `locations` slice — a user-managed set of physical storage places (name + optional note) with add / edit / delete-if-empty — and let a Spool be optionally assigned to a Location, surfaced on the spool add/edit forms and reassignable from the spool detail card.

**Slice / context:**
Fourth vertical slice, first new aggregate since `spools`. Foundation, `materials`
(01), `spools` core (03a) and `spools` ops (03b) are shipped. `Location` is already
in the glossary ("A physical place a Spool is stored (drybox, shelf). A plain
storage place in v1; a Location may later be sensor-monitored — deferred"). This
slice builds the **plain storage place**; the humidity/drybox specialisation stays
deferred (no `kind`/sensor fields — see the humidity scope note). It follows the
`materials` slice as the CRUD template (hexagonal: pure domain aggregate, API/SPI
ports, SQLx adapter, htmx driving adapter) and the established conventions: opaque
id promoted to `shared/`, ULID generated in the persistence adapter, validated name
newtype, i18n en+fr parity, PostgreSQL + testcontainers (ADR-0003).

**Desired behavior:**

- **Create a location.** Operator adds a Location with a **name** (required) and an
  optional **note** (free text). Name is trimmed and must not be blank/whitespace
  (rejected as a 422 form error, mirroring `MaterialName`). Note is trimmed; an
  empty/whitespace note is stored as absent (`None`). There is **no seed** — the
  location set starts empty and is entirely user-built.
- **Edit a location.** Operator changes a Location's name and/or note. Same name
  invariant. Unknown id on edit is **404** (the SPI reports a 0-row update as
  not-found — do **not** reintroduce the TD-003 silent-no-op class in this slice).
- **Delete a location — only if empty.** A Location with **zero** spools assigned
  can be deleted. A Location with one or more assigned spools is **rejected** with a
  **409** and an inline htmx message naming the count ("N spools stored here —
  reassign them first"). The emptiness guard is enforced in the use case (via an SPI
  count), never only in the UI. Deleting an unknown id is **404**.
- **List locations.** A page (`GET /locations`) lists all locations by name, each
  row showing name, note, and its current assigned-spool count, with edit and
  (empty-only) delete controls. New sidebar nav entry for Locations.
- **Assign a Spool to a Location (optional).** A Spool may reference **at most one**
  Location, or none. The FK is nullable — every pre-existing spool stays unassigned.
  Assignment is offered:
  - on the **add-spool** form (optional Location `<select>`; blank ⇒ unassigned),
  - on the **edit-spool** form (same select, preselecting the current location),
  - on the **spool detail card** as a reassign control (htmx, same fragment-swap
    pattern as the weight/archive ops), including an "unassign" (blank) choice.
- **Display assignment.** The spool detail shows the assigned Location name (or an
  "unassigned" label); the spool **list read model** carries the Location name as a
  nullable primitive so a row can show it. This name is populated by a join **in the
  SQLx adapter** — the `spools` slice must **not** import the `locations` module
  (slice isolation; same rule already used for the Material name/density fields).
- **Unknown location on assignment.** Assigning a spool to a non-existent Location id
  is a not-found outcome surfaced as **404** (ids come from a rendered select, so this
  is defensive). See *Two-FK disambiguation* below — the spools adapter must not
  misreport an unknown-location FK violation as `UnknownMaterial`.
- **i18n:** all new UI strings (location form labels, buttons, the delete-blocked
  message, the "unassigned" label, nav entry) via the en/fr catalogs (ADR-0001); new
  keys added to **both** locales; no hardcoded strings.

**Key interfaces:** (glossary + API/SPI terms — no file paths)
- `LocationId` (`shared/`) — opaque id newtype promoted to the shared kernel (the
  `spools` slice references it, exactly like `MaterialId`); `new(impl Into<String>)`,
  `as_str()`.
- `LocationName` — validated name newtype (trim, reject blank → new
  `DomainError::BlankLocationName`); `new(impl Into<String>) -> Result`, `as_str()`.
- `Location { id: LocationId, name: LocationName, note: Option<String> }` and
  `NewLocation { name: LocationName, note: Option<String> }` (id/derived fields
  assigned by the repository).
- `LocationsUseCases` (API port): `list`, `add(NewLocation)`, `edit(Location)`,
  `delete(LocationId)`.
- `LocationRepository` (SPI port): `list`, `insert(NewLocation)`, `update(Location)`,
  `delete(LocationId)`, `count_spools(&LocationId) -> u64`. `RepositoryError` mirrors
  the materials/spools SPI error with at least `Backend`, `NotFound(LocationId)`, and
  a `Domain(#[from] DomainError)` arm; delete surfaces the not-empty case as a domain
  error (`DomainError::LocationInUse { count }`) mapped to 409 at the edge.
- `spools` additions: `NewSpool`/`Spool` gain `location_id: Option<LocationId>`; the
  `SpoolsUseCases` port gains `assign_location(SpoolId, Option<LocationId>) -> Result`;
  the `SpoolListItem`/`SpoolDetail` read models gain `location_name: Option<String>`
  (primitive). `SpoolRepository::update` already carries the aggregate, so it persists
  `location_id`; add a dedicated assign path or reuse `find`→mutate→`update`.

**Acceptance criteria (the done contract):**
- Domain unit tests: `LocationName` rejects blank/whitespace and trims; `NewLocation`
  round-trips note `Some`/`None`; delete use case returns `LocationInUse` when
  `count_spools > 0` and `Ok` when `0`; edit of an unknown id → `NotFound`.
- SPI integration (testcontainers): insert→list; update persists name+note; update of
  unknown id → `NotFound` (0-row guard); `count_spools` reflects assigned spools;
  delete blocked while a spool references the location, succeeds after unassign; the
  spool list/detail join shows the assigned location name and `None` when unassigned.
- Spool assignment: adding a spool with a location persists it; editing reassigns;
  detail-card reassign swaps the fragment and persists; blank ⇒ unassigned; unknown
  location id → 404 (not misreported as unknown material).
- Web: `GET /locations` renders rows with name/note/count and edit + (empty-only)
  delete; `POST /locations` blank name → 422; `DELETE /locations/{id}` on a non-empty
  location → 409 with the count message; on an empty one → row removed.
- e2e journey: add location → add/assign a spool to it → attempt delete (blocked, 409)
  → unassign the spool → delete location (ok).
- i18n: en+fr key parity for every new string; no raw i18n keys leak into rendered
  HTML (assert as in the materials/spools tests).
- All existing tests stay green; clippy + offline build clean.

**Non-goals / out of scope (YAGNI):**
- Location **kind/type** (shelf/drybox/other) and any sensor/humidity field — deferred
  to the humidity slice.
- Filtering the spool **list** by location; location-based dashboard aggregates
  (dashboard is its own later slice).
- Multi-location per spool, capacity limits, location hierarchy.

**Design notes / constraints:**
- **Two-FK disambiguation.** After adding `spools.location_id`, the spools SQLx
  adapter has two FKs (`material_id`, `location_id`). The current `backend()` mapper
  blindly maps *any* foreign-key violation to `UnknownMaterial`. It must inspect the
  violated constraint (e.g. `db.constraint()`) to map the material FK →
  `UnknownMaterial` and the location FK → a new `UnknownLocation(LocationId)` (→404);
  an unrecognised constraint stays `Backend`. A regression test should cover an
  unknown-location assignment producing the location outcome, not the material one.
- **Migration** `0004_locations.sql`: create `locations (id TEXT PRIMARY KEY, name
  TEXT NOT NULL, note TEXT NULL)`; `ALTER TABLE spools ADD COLUMN location_id TEXT NULL
  REFERENCES locations(id)`; index on `spools(location_id)`. Nullable column keeps the
  existing rows valid. (No `ON DELETE` cascade — the app-level emptiness guard owns
  deletion semantics; DB default `NO ACTION` is a backstop.)
- **Slice isolation:** `spools` carries `location_name` as a primitive filled by an
  adapter join; no `spools`→`locations` domain import. `LocationId` lives in `shared/`.
- **Error hygiene:** new 500 arms follow the existing handlers (TD-005 class — raw
  strings to client — is accepted debt for v1; do not widen gratuitously, but matching
  the existing pattern is fine).
- Follow ADR-0001 (EN code / i18n UI), ADR-0002 (2-crate workspace), ADR-0003
  (PostgreSQL + testcontainers). Keep `.sqlx/` offline cache updated (`cargo sqlx
  prepare`) so CI `SQLX_OFFLINE=true` builds.

**Tech-debt touchpoints:** closes the TD-003 *class* for the new locations `update`
(0-row⇒NotFound from the start); does not resolve TD-003 for materials. May file a
follow-up if the detail-card reassign needs a `spools.detail.location.*` key set.
