> _AI-generated brief (Claude Code), reviewed before delegation. Tracks GitHub issue #35._

## Agent Brief

**Category:** feature
**Summary:** From Settings, export the whole instance to a versioned JSON file and import such a file to recreate an instance (full replace).

**Slice / context:** Backup/restore slice, hosted on the Settings screen (slice `10`/#27). Filature is deployed one PostgreSQL database per instance (e.g. `filature_gwen`, `filature_zig`), so duplicating or restoring an instance is a real need. Depends on the Settings screen existing.

**Desired behavior:**
- **Export** produces a single JSON document containing all business content of the instance — Spools, Materials, Manufacturers, storage Locations, and instance settings — plus a top-level **format/version** field.
- **Import** accepts a previously exported JSON and **replaces** the entire instance content with it (wipe + recreate). There is **no merge** with existing data.
- Import is **atomic**: it runs in a single transaction; any failure rolls back and leaves the instance untouched (never half-overwritten).
- Import **validates** the document before applying: the format/version must be understood (an unknown/incompatible version is refused with a clear message, no partial write) and the payload must satisfy the expected schema.
- The UI **confirms** the destructive nature before importing (import erases everything).
- Upload is **size-limited**; oversized or malformed uploads are rejected.

**Key interfaces:** (glossary + API/SPI terms)
- An **instance export/import API port**: `export()` → versioned document; `import(document)` → validated full replace.
- SPI persistence capability to read all aggregates and to atomically clear + repopulate them (PostgreSQL transaction — [ADR-0003](../adr/0003-postgresql-persistence.md)).
- (De)serialisation lives in a testable layer independent of the web adapter.

**Acceptance criteria:**
- [ ] Export yields a versioned JSON covering Spools, Materials, Manufacturers, Locations, and settings.
- [ ] Importing an exported file recreates an equivalent instance (full replacement).
- [ ] An unknown/incompatible version is refused clearly with no data corruption.
- [ ] Import is atomic — a failure mid-import rolls back with the prior content intact.
- [ ] The UI requires explicit confirmation before an import.
- [ ] Upload has a validated size limit; malformed input is rejected.
- [ ] **A security review is completed before merge** (import is untrusted external input): schema validation, size cap, transactional safety, and confirmation are all verified.
- [ ] Strings in fr + en.

**Out of scope:**
- Partial/selective export or import (single entity kind, single spool).
- Merge/upsert semantics — replace only.
- Scheduled/automatic backups.
- Cross-instance migration tooling beyond this JSON round-trip.

**References:**
- Issue: #35 · depends on: #27 (Settings screen)
- Security: run `security-review`; log residual risk in `docs/security/accepted-risks.md` if any
- ADRs: `docs/adr/0003-postgresql-persistence.md`, `docs/adr/0001-language-and-i18n.md`
- Glossary: `docs/glossary.md` (Spool, Material, Manufacturer, Location)
