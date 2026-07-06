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
| **Spool** | Bobine | A single physical reel of filament held in stock. Made of one Material, has a real colour, a diameter, and a remaining amount of filament. Carries its own lifecycle (see Spool Status). |
| **Material** | Matériau | An entry in the material referential (PLA, PETG, ASA, PA-CF, …). The single source of truth for a filament type's physical & handling properties: density, drying parameters, and humidity Sensitivity. Seeded at startup, editable. Referenced by Spools. |
| **Location** | Rangement | A physical place a Spool is stored (drybox, shelf). A plain storage place in v1; a Location may later be sensor-monitored (deferred — see below). |

## Spool measurements & derived values

| Term (canonical) | FR label | Definition |
|---|---|---|
| **Net Weight** | Poids net | The filament weight recorded for a Spool, taken from the manufacturer's label (e.g. a "1 kg" spool ⇒ 1000 g). Entered directly, not derived from a weighing. The baseline the remaining amount is measured against. See [ADR-0004](adr/0004-net-weight-no-tare.md). |
| **Remaining Weight** | Reste | The current filament weight left on a Spool, in grams. Updated by direct entry ("remaining is now X g") or by recording a consumed amount ("used Y g"). Zero ⇒ the Spool becomes Empty. See [ADR-0004](adr/0004-net-weight-no-tare.md). |
| **Remaining Ratio** | Pourcentage restant | Remaining Weight ÷ Net Weight, as a percentage. Drives low-stock signalling. |
| **Remaining Length** | Longueur restante | The Remaining Weight expressed as metres of filament, derived from the Material's density and the Spool's diameter. A presentation of the same quantity as Remaining Weight, not a stored value. |
| **Stock Value** | Valeur du stock | The monetary worth of remaining filament: summed over Spools of `(Remaining Weight ÷ Net Weight) × Price Paid`. |
| **Diameter** | Diamètre | The filament diameter of a Spool. One of the two market standards: **1.75 mm** or **2.85 mm**. Used with Material density to derive Remaining Length. |
| **Colour** | Couleur | A Spool's real filament colour: a hex value (`#RRGGBB`) with an optional free-text name (e.g. `#1A9E4B` "vert sapin"). |
| **Price Paid** | Prix payé | The amount paid for a Spool when acquired (full-spool price). Feeds Stock Value. |

## Enumerations

| Term (canonical) | FR label | Definition |
|---|---|---|
| **Spool Status** | Statut de bobine | The lifecycle state of a Spool. One of: **Sealed** (Scellée — unopened, full), **Open** (Ouverte — in use), **Empty** (Vide — Remaining Weight reached 0), **Archived** (Archivée — retired from active stock, kept for history). |
| **Sensitivity** | Sensibilité | A Material's susceptibility to humidity. One of **Low**, **Medium**, **High**. Determines the humidity threshold at which stored Spools of that Material are considered at risk. Stored now; only consumed by the deferred Humidity feature. |

## Deferred terms (Humidity feature — post-v1, no sensors yet)

Recorded so the language is stable when the feature is picked up; **not modelled
or built in v0-v1**. See `docs/product/brief.md` scope note.

| Term (canonical) | FR label | Definition |
|---|---|---|
| **Drybox** | Drybox | A Location that is humidity-monitored by a sensor. A specialisation of Location once monitoring exists. |
| **Humidity Reading** | Relevé d'humidité | A timestamped measurement (relative humidity %, temperature) from a Drybox's sensor. |
| **Humidity Threshold** | Seuil d'humidité | The relative-humidity ceiling above which a Material is at risk, derived from its Sensitivity. |
| **Drybox Status** | Statut de drybox | A Drybox's risk state (Stable / Watch / Dry) — the worst status among the Materials stored in it, given the latest Humidity Reading versus each Material's Humidity Threshold. |
