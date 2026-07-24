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
| **Print Head** | Tête | A physical toolhead carrying one filament path. Every Printer has N Print Heads (N ≥ 1, default 1). On Bambu printers its Feed Mode determines whether it contributes a direct Slot or draws from the shared AMS Unit pool. |
| **Feed Mode** | Mode d’alimentation | A per-Print Head Bambu setting. **Direct** contributes one direct-spool Slot; **AMS-fed** draws from the shared AMS Unit pool and contributes no direct Slot. |
| **AMS Unit** | Unité AMS | One uniform four-Slot AMS attached to a Bambu Printer. A Printer has zero to four ordered AMS Units; all AMS-fed heads share their Slots, without head-to-unit routing. |
| **Filament Module** | Module | A multi-spool feeding unit. Kinds: **MMU** (Prusa — 5 Slots) and **Multi-Slot** (automatic multi-material changer with a fixed selected Slot count). Bambu AMS is represented by AMS Units, not a Filament Module. INDX and Multi-colour unit are represented by Multi-Slot; the former Tool Changer concept is represented by Print Head count. |
| **Slot** | Emplacement de bobine | A single filament position on a Printer or its Module, holding at most one loaded Spool at a time; may be empty. Has a stable key within its Printer so assignments survive an edit that keeps the layout. (Not to be confused with the Settings › Emplacements tab, which manages storage Locations.) |
| **Loaded Spool / Loading** | Bobine chargée / Charger | The assignment of a Spool to a Slot. A Spool is loaded into **at most one Slot across all Printers** — two Printers can never share a Spool, nor two Slots of one Printer (exclusivity). Loading is independent of the Spool's Location and Status. Only **Sealed** or **Open** Spools can be loaded; a Spool that becomes **Empty** or **Archived** is automatically unloaded. |

## Machine connectivity (slices `22a`/`22b`)

Introduced by the Machine Link feature (live printer status). Design decisions
recorded during discovery on 2026-07-22.

| Term (canonical) | FR label | Definition |
|---|---|---|
| **Machine Link** | Connexion machine | An optional network configuration attached to a Printer, linking the declared Printer to its physical machine so Filature can query its state. One kind per Printer Brand: **PrusaLink** (host + API key), **Moonraker** (API URL — for Other printers marked as Klipper machines), **Bambu LAN** (host + LAN access code + serial number). A Printer without a Machine Link behaves as before. |
| **Machine Status** | Statut machine | The instantaneous state reported by a Printer's physical machine through its Machine Link: a Machine State, and — when relevant — job progress (percent, remaining time), the current job name, and nozzle/bed temperatures (active Print Head; first head if the machine does not report which is active). Never persisted; fetched on demand. |
| **Machine State** | État machine | The top-level state within a Machine Status. One of: **Offline** (Hors-ligne — Machine Link configured but the machine is unreachable), **Idle** (Repos), **Printing** (Impression), **Paused** (Pause), **Error** (Erreur). |
| **Farm Activity** | Activité du parc | The dashboard panel listing every Printer that has a Machine Link, each with its live Machine Status — no Slots or Spools shown. Printers without a Machine Link appear only on the Printers page. |

## AMS spool sync (slice `23`)

Introduced by the AMS spool sync feature. Lets Filature read what a Bambu AMS
physically holds and reconcile it against the operator's Spools, removing the
double entry between the physical AMS and manual Slot loading (`15b`). Bambu
only — no other Machine Link kind reports per-tray filament data.

| Term (canonical) | FR label | Definition |
|---|---|---|
| **AMS Tray** | Bac AMS | The live filament reading a Bambu AMS reports for one of its four positions via MQTT: filament type, colour, sub-brand (e.g. `PLA Basic`), a coarse remaining percentage, and an **AMS Tag UID**. A Tray is the machine's live view of an AMS-Unit **Slot**; it is never persisted. |
| **AMS Tag UID** | Identifiant RFID AMS | The RFID tag UID a Bambu AMS reports for a Tray (`tag_uid`). Genuine Bambu spools carry a unique UID; third-party spools report `0000…` (absent). Once an operator confirms a match, the UID is **memorized on the Spool**, making later syncs a certain match. |
| **AMS Reconciliation** | Réconciliation AMS | The operator-confirmed process of matching live AMS Trays to Filature Spools and loading each into its AMS-Unit Slot. The system **suggests** a Spool per Tray — by AMS Tag UID first, else by type + colour (+ sub-brand) among loadable Spools — and the operator confirms or corrects. Never auto-creates a Spool; never silently overwrites remaining weight (Filature's weighed weight stays authoritative, ADR-0004 — an AMS/Filature discrepancy is surfaced for the operator to resolve). Reuses the `15b` loading rules (exclusivity, Sealed/Open only). |

## Deferred terms (Humidity feature — post-v1, no sensors yet)

Recorded so the language is stable when the feature is picked up; **not modelled
or built in v0-v1**. See `docs/product/brief.md` scope note.

| Term (canonical) | FR label | Definition |
|---|---|---|
| **Drybox** | Drybox | A Location that is humidity-monitored by a sensor. A specialisation of Location once monitoring exists. |
| **Humidity Reading** | Relevé d'humidité | A timestamped measurement (relative humidity %, temperature) from a Drybox's sensor. |
| **Humidity Threshold** | Seuil d'humidité | The relative-humidity ceiling above which a Material is at risk, derived from its Sensitivity. |
| **Drybox Status** | Statut de drybox | A Drybox's risk state (Stable / Watch / Dry) — the worst status among the Materials stored in it, given the latest Humidity Reading versus each Material's Humidity Threshold. |
