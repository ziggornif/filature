# Glossary

> Maintained by `ubiquitous-language`. Domain terms only — no implementation detail.
> Path recorded in `.harness/config.yml` so all skills find it without hardcoded convention.

Single bounded context (a personal filament stock tool). **Canonical terms are
English** — Rust types, SQL tables, and slice folder names use these words
verbatim. The **UI is internationalised** (en + fr shipped, extensible); the
`FR label` column is the built-in French translation of each user-facing term,
not a second canonical name. See [ADR-0001](adr/0001-language-and-i18n.md).

## Core entities

| Term (canonical) | FR label | Definition |
|---|---|---|
| **Spool** | Bobine | A single physical reel or holder-less refill of filament held in stock. Made of one Material, may have a recorded colour, has a diameter, and a remaining amount of filament. Carries its own lifecycle (see Spool Status). |
| **Material** | Matériau | An entry in the material referential (PLA, PETG, ASA, PA-CF, …). The single source of truth for a filament type's physical & handling properties: density, drying parameters, and humidity Sensitivity. Seeded at startup, editable. Referenced by Spools. |
| **Location** | Rangement | A physical place a Spool is stored (drybox, shelf). A plain storage place in v1; a Location may later be sensor-monitored (deferred — see below). |
| **Manufacturer** | Fabricant | The brand that produced a Spool (Prusament, Polymaker, …). An entry in a referential seeded at startup from a curated subset of the OpenPrintTag brand database, editable (add/delete). A Spool optionally references one; a Manufacturer that any Spool references cannot be deleted. Distinct from Material — the same Material (e.g. PLA) is sold by many Manufacturers. |

## Spool measurements & derived values

| Term (canonical) | FR label | Definition |
|---|---|---|
| **Net Weight** | Poids net | The filament weight recorded for a Spool, taken from the manufacturer's label (e.g. a "1 kg" spool ⇒ 1000 g). Entered directly, not derived from a weighing. The baseline the remaining amount is measured against. See [ADR-0004](adr/0004-net-weight-no-tare.md). |
| **Remaining Weight** | Reste | The current filament weight left on a Spool, in grams. Updated by direct entry ("remaining is now X g") or by recording a consumed amount ("used Y g"). Zero ⇒ the Spool becomes Empty. See [ADR-0004](adr/0004-net-weight-no-tare.md). |
| **Remaining Ratio** | Pourcentage restant | Remaining Weight ÷ Net Weight, as a percentage. Drives low-stock signalling. |
| **Low-Stock Threshold** | Seuil de stock bas | The instance-wide percentage of Remaining Ratio at or below which a non-empty Spool is considered soon empty. Defaults to 15% and is configurable in Settings. Distinct from the deferred Humidity Threshold. |
| **Remaining Length** | Longueur restante | The Remaining Weight expressed as metres of filament, derived from the Material's density and the Spool's diameter. A presentation of the same quantity as Remaining Weight, not a stored value. |
| **Stock Value** | Valeur du stock | The monetary worth of remaining filament: summed over Spools of `(Remaining Weight ÷ Net Weight) × Price Paid`. |
| **Diameter** | Diamètre | The filament diameter of a Spool. One of the two market standards: **1.75 mm** or **2.85 mm**. Used with Material density to derive Remaining Length. |
| **Colour** | Couleur | A Spool's optional real filament colour: a normalized hex value (`#RRGGBB`) or Transparent. Its name is derived from the matching preset, otherwise from the upper-cased hex value; it is never free-typed. |
| **Price Paid** | Prix payé | The amount paid for a Spool when acquired (full-spool price). Feeds Stock Value. |

## Enumerations

| Term (canonical) | FR label | Definition |
|---|---|---|
| **Spool Status** | Statut de bobine | The lifecycle state of a Spool. One of: **Sealed** (Scellée — unopened, full), **Open** (Ouverte — in use), **Empty** (Vide — Remaining Weight reached 0), **Archived** (Archivée — retired from active stock, kept for history). |
| **Spool Type** | Type de bobine | The physical form of stock. **Complete** is filament supplied on a holder; **Recharge** is a refill supplied without one. |
| **Sensitivity** | Sensibilité | A Material's susceptibility to humidity. One of **Low**, **Medium**, **High**. Determines the humidity threshold at which stored Spools of that Material are considered at risk. Stored now; only consumed by the deferred Humidity feature. |

## Printers & filament loading

Introduced by the Printers feature (slices `15a`/`15b`). Source of truth for the
UI: `init_assets/design_handoff_filature/Filature.dc.html` (Imprimantes 3D page,
printer form, Settings › Imprimantes tab).

| Term (canonical) | FR label | Definition |
|---|---|---|
| **Printer** | Imprimante | A physical 3D printer owned by the operator. Has a Printer Brand, a Printer Model, one or more Print Heads, an optional Filament Module, and one or more Slots. Spools are loaded into its Slots. User-managed (add / edit / delete). |
| **Printer Brand** | Marque | A fixed enumeration deciding a Printer's available Module options and its card accent colour: **Bambu Lab** (green liner), **Prusa** (orange liner), **Other** (neutral/violet liner). Distinct from Manufacturer — a Manufacturer is a *filament* brand, a Printer Brand is a *machine* brand. |
| **Printer Model** | Modèle | The specific machine model. Curated (fixed list) per Brand for Bambu Lab (A1 mini, A1, A2L, P1P, P1S, P2S, X1 Carbon, X2D, H2S, H2D, H2C) and Prusa (MINI+, MK3 / MK3S / MK3S+, MK4S, CORE One+, CORE One L, XL); free text for Other. Stored as plain text. |
| **Print Head** | Tête | A physical toolhead carrying one filament path. Every Printer has N Print Heads (N ≥ 1, default 1). With multiple heads, each head is an independent direct-spool Slot. |
| **Filament Module** | Module | A multi-spool feeding unit that adds Slots to a single-head Printer. Kinds: **AMS** (Bambu — 4 Slots plus one external Slot), **MMU** (Prusa — 5 Slots), and **Multi-Slot** (brand-agnostic automatic multi-material changer with a fixed selected Slot count). INDX and Multi-colour unit are represented by Multi-Slot; the former Tool Changer concept is represented by Print Head count. A single-spool Printer has no Module and exactly one Slot. |
| **Slot** | Emplacement de bobine | A single filament position on a Printer or its Module, holding at most one loaded Spool at a time; may be empty. Has a stable key within its Printer so assignments survive an edit that keeps the layout. (Not to be confused with the Settings › Emplacements tab, which manages storage Locations.) |
| **Loaded Spool / Loading** | Bobine chargée / Charger | The assignment of a Spool to a Slot. A Spool is loaded into **at most one Slot across all Printers** — two Printers can never share a Spool, nor two Slots of one Printer (exclusivity). Loading is independent of the Spool's Location and Status. Only **Sealed** or **Open** Spools can be loaded; a Spool that becomes **Empty** or **Archived** is automatically unloaded. |

## Deferred terms (Humidity feature — post-v1, no sensors yet)

Recorded so the language is stable when the feature is picked up; **not modelled
or built in v0-v1**. See `docs/product/brief.md` scope note.

| Term (canonical) | FR label | Definition |
|---|---|---|
| **Drybox** | Drybox | A Location that is humidity-monitored by a sensor. A specialisation of Location once monitoring exists. |
| **Humidity Reading** | Relevé d'humidité | A timestamped measurement (relative humidity %, temperature) from a Drybox's sensor. |
| **Humidity Threshold** | Seuil d'humidité | The relative-humidity ceiling above which a Material is at risk, derived from its Sensitivity. |
| **Drybox Status** | Statut de drybox | A Drybox's risk state (Stable / Watch / Dry) — the worst status among the Materials stored in it, given the latest Humidity Reading versus each Material's Humidity Threshold. |
