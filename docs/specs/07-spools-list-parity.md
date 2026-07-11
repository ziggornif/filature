## Agent Brief — 07 Spools list · toolbar & filter parity

**Category:** feature
**Summary:** Close the remaining gaps between the spools-list screen and the hi-fi
handoff toolbar: a free-text search (brand + colour), a Manufacturer filter, a
Location (rangement) filter, and the "X sur Y affichées" result counter. Card
and table views already exist (slice 06 — card view) — this slice only extends
the filter/toolbar surface that feeds both.

**Slice / context:**
The spools list (`spools.html`, `_spool_rows.html`, `_spool_row.html`,
`_spool_card.html`) renders a table **and** a card grid, toggled client-side via
a CSS radio. Filtering is an htmx form: each control `hx-get /spools/rows`,
swapping `#spools-table-body` and re-rendering the card grid out-of-band, plus an
oob `#stock-value`. Existing filters: **Material**, **Status**, **Sort**
(`SpoolQuery` → `SpoolFilter`/`SpoolSort` in `domain/spools/ports/spi.rs`). The
manufacturers and locations referentials already exist and are already loaded as
`<select>` options on the add form (`manufacturer_options`, `location_options` in
`web/spools.rs`).

The handoff toolbar (`init_assets/design_handoff_filature/Filature.dc.html`,
BOBINES section) shows four things this screen does **not** yet have:
1. a search input — placeholder "Rechercher marque, couleur…";
2. a **Toutes marques** (manufacturer) filter select;
3. a **Tous rangements** (location) filter select;
4. a subtitle counter — "`{filteredCount}` sur `{totalCount}` affichées".

**Desired behavior:**

- **Search.** A text input filters the list by a case-insensitive substring match
  against **manufacturer name** and **colour name** (the placeholder's two fields).
  Empty = no constraint. Debounced htmx trigger (`keyup changed delay:300ms`),
  same `hx-get /spools/rows` target contract as the other controls.
- **Manufacturer filter.** A `<select>` (default option "Toutes marques") listing
  every manufacturer from the referential; selecting one restricts the list to
  spools attributed to it. "Unattributed" spools (null manufacturer) are excluded
  when a specific brand is chosen.
- **Location filter.** A `<select>` (default "Tous rangements") listing every
  location; selecting one restricts to spools stored there. Unassigned spools are
  excluded when a specific location is chosen.
- **Result counter.** The header subtitle shows `<filtered> sur <total> affichées`
  where `filtered` = rows after the active filters and `total` = all spools
  (unfiltered, non-archived). Both numbers re-render on every filter change (oob,
  alongside `#stock-value`). Mono digits, muted text (see handoff).
- All four combine (AND) with the existing Material/Status/Sort filters. Selected
  values persist across a full page reload (echoed back as `selected` / `value`,
  same pattern as `selected_material`).

**Domain / port changes:**

- Extend `SpoolFilter` (`domain/spools/ports/spi.rs`) with:
  `manufacturer_id: Option<ManufacturerId>`, `location_id: Option<LocationId>`,
  and `search: Option<String>` (or a small `text` newtype — implementer's call,
  but the match is name-based, case-insensitive).
- The list SPI already returns `manufacturer_name` / `location_name` on
  `SpoolListItem`; the **filter** pushes down to SQL (`WHERE`), it is not a
  post-filter in Rust — keep parity with how material/status filter today
  (`persistence/spools.rs`).
- **Total count**: add a cheap count (unfiltered) path — either a dedicated
  `count()` on the repo or reuse `list` length at the call site. Filtered count =
  length of the already-fetched filtered list (no extra query).

**Web / template changes:**

- `SpoolQuery`: add `search`, `manufacturer_id`, `location_id` fields + map them
  in `to_filter()` (mirror the `filter(|s| !s.is_empty())` guard).
- `list_page` + `rows` handlers: load manufacturer/location options (already have
  helpers), compute `filtered_count` + `total_count`, insert into context.
- `spools.html`: add the search input + two selects to `.spools-filter-bar`; add
  the counter to the header. `_spool_rows.html`: add oob counter span(s) next to
  the oob stock-value.
- **i18n** (en + fr, ADR-0001 — no hardcoded strings): `spools.filter.search`
  (placeholder), `spools.filter.all_manufacturers` ("Toutes marques"),
  `spools.filter.location` + `spools.filter.all_locations` ("Tous rangements"),
  `spools.count.showing` (a templated "{filtered} sur {total} affichées" — check
  how the catalog handles interpolation, or compose from parts as the stock-value
  line does).

**Out of scope:**
- The card/table view toggle and card layout (done, slice 06).
- Inline weight edit inside the table (that is the 03b operational surface).
- The "Réinitialiser" reset-filters button and empty-state illustration from the
  handoff — nice-to-have, can be a follow-up; note if skipped.

**Acceptance / tests (house style — render assertions + filter unit tests):**
- Search narrows by brand and by colour name; empty search = all rows.
- Manufacturer/location filters narrow correctly and exclude null-attribution rows.
- Counter shows `filtered sur total`; equal when no filter is active.
- Selected filter values survive a reload (echoed as selected/value).
- No raw i18n keys leak; fr + en both resolve.
- SQL push-down (filter is in the query, not a Rust post-filter).
