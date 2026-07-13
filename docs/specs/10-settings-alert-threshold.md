> _AI-generated brief (Claude Code), reviewed before delegation. Tracks GitHub issue #27._

## Agent Brief

**Category:** feature
**Summary:** Add a Settings screen and make the low-stock alert threshold a global, configurable percentage.

**Slice / context:** New Settings slice. Today the low-stock signal ("bientôt vide") fires on a threshold that is hard-coded; there is no place to change it, and no Settings surface at all (nav is Dashboard / Spools / Materials / Manufacturers / Locations). The dashboard already displays a "seuil 15 %" in the design, so 15 % is the sensible default. This screen is also the host for the instance export/import slice (`12`/#35).

**Desired behavior:**
- A new **Settings** navigation entry opens a dedicated screen.
- The screen exposes a **Low-Stock Threshold**: a global percentage of Net Weight below which a Spool is considered nearly empty. Editable, persisted, reloaded on startup.
- Changing it re-derives low-stock signalling everywhere it is consumed — the dashboard Alerts count and the "Bientôt vides" panel reflect the configured value, not a constant.
- If never set, the effective value defaults to **15 %**.
- The threshold is a single instance-wide setting — **not** per Spool.

**Key interfaces:** (glossary + API/SPI terms)
- **Low-Stock Threshold** — new glossary term (the percentage of Remaining Ratio under which a Spool is low-stock). Distinct from the deferred Humidity Threshold.
- An **instance-configuration** concept: an API port to read/update global settings, backed by an SPI persistence capability (a settings row/table in PostgreSQL — see [ADR-0003](../adr/0003-postgresql-persistence.md)).
- The low-stock determination (currently driven by **Remaining Ratio** vs a constant) reads the configured threshold instead.

**Acceptance criteria:**
- [ ] A Settings nav entry and screen exist and are reachable.
- [ ] The threshold is editable as a percentage, persisted, and survives a restart.
- [ ] Dashboard Alerts and "Bientôt vides" use the configured threshold.
- [ ] With no stored value, the effective threshold is 15 %.
- [ ] Invalid input (out of 0–100) is rejected with a clear message.
- [ ] Settings strings are in the fr + en catalogs; a render test covers a non-default locale.

**Out of scope:**
- Export/import of the instance — slice `12`/#35 (lives on this screen but is its own unit).
- Per-Spool thresholds.
- Any humidity/drybox settings (deferred post-v1).
- Theme/locale controls (already handled elsewhere; not part of this slice).

**References:**
- Issue: #27 · downstream: #35 (export/import)
- ADRs: `docs/adr/0003-postgresql-persistence.md`, `docs/adr/0001-language-and-i18n.md`
- Glossary: `docs/glossary.md` (Remaining Ratio, Spool Status)
- Design decisions: `docs/design.md`
