# Design

> Decisions about the product's interface and aesthetic character.
> **Source of truth for the UI is the hi-fi handoff** in
> `init_assets/design_handoff_filature/` (README.md = spec, Filature.dc.html =
> visual reference, `support.js` = proto runtime, NOT an implementation target).
> This file records the load-bearing decisions and the deltas from that handoff.
> No prototype UI branch was run — the design is already hi-fi.

## Character

A **workshop instrument** kept open in a tab, not a consumer app. Dense,
legible, calm. The only vivid colour comes from the data (each spool's real
filament colour); semantic colour (green/amber/red) is reserved for status,
never decoration.

## Visual language (from the handoff — treat as definitive)

- **Themes light AND dark at parity.** Default = OS `prefers-color-scheme`, plus
  a persistent manual toggle. CSS tokens on `:root`, overridden via
  `html[data-theme="light|dark"]`; the attribute is set **server-side at render**
  from a cookie so the OS default is honoured on first paint.
- **Palette:** neutral warm greys (workshop, not clinical blue), 3 surface
  levels. One discreet accent (`#5b5563` slate default). Semantic tokens
  (`--ok`/`--warn`/`--danger`) with a per-theme variant. Full token values (light
  + dark) are in the handoff README §Design Tokens — copy them verbatim.
- **Typography:** IBM Plex Sans (UI) + IBM Plex Mono (all numbers, units, codes).
  **Monospace for every figure** (g, m, %, %HR, °C, €) to align columns — the
  instrument feel. Self-host the woff2 (embedded in the binary, no network).
- **Signature components:** filament colour **chip** (always a
  `--border-strong` ring; hatch pattern for "transparent"); **remaining-weight
  gauge** (bar whose fill goes neutral → amber under low threshold → red under
  10% → grey when empty, with g + % in mono).
- **Icons:** Feather/Lucide light set, inline SVG.
- **Radii/spacing:** cards 11–12px, controls 6–8px, chips circular, status pills
  20px; grid gap 14px. Exact values in the handoff.
- **CSS units: px, not rem/em** (decided July 2026, see
  `init_assets/design_handoff_filature/BRIEF_imprimantes_3d.md` §2). This is a
  fixed-dimension instrument layout, not editorial content meant to rescale with
  browser font size; every handoff token is in px — reproduce them as px. If a
  concrete accessibility need surfaces later (user zoom, system font-size
  preference), re-evaluate case by case (typically `font-size` in rem, other
  dimensions staying px).

## Screens (6, specified in the handoff README §Screens)

Dashboard · Spools list (table + card views, inline weight edit) · Spool detail ·
Add/Edit form (**wizard 2 écrans** — état → détails ; poids net par presets, **pas de tare**) · Materials table ·
**Humidity panel (deferred — post-v1, no sensors).**

## Interaction principles (htmx, server-rendered)

- Every self-updating unit is an **autonomous htmx fragment** re-rendered in
  place (a spool row, a card, the list panel). No SPA, no custom JS beyond htmx,
  no build step.
- **Filtering** = each control `hx-get` → swaps the `<tbody>`/grid (light debounce
  on search). **Inline weight edit** = `hx-get` edit fragment → `hx-put` returns
  the re-rendered row; Enter commits, Esc cancels; remaining→0 flips status to
  Empty. **Theme** = cookie + `data-theme` on `<html>`.
- State lives server-side; handlers return HTML fragments. The proto's in-memory
  JS state is NOT ported.

## Deltas from the handoff

- **htmx via CDN (not embedded).** htmx is loaded from jsdelivr with an SRI
  `integrity` hash + `crossorigin`, rather than vendored into the binary. This is
  a deliberate runtime network dependency on the frontend (trades offline
  self-sufficiency for a lighter binary + browser CDN caching). Self-hosted woff2
  fonts stay embedded; only htmx is CDN-loaded.
- **i18n (ADR-0001).** The handoff assumed a French UI; the real UI is
  internationalised (en + fr shipped, extensible). No hardcoded strings — every
  user-facing label comes from a per-locale catalog. htmx fragments must render
  in the active locale (locale resolved server-side, like the theme). Render
  tests cover a non-default locale so missing keys fail at `cargo test`.
- **Humidity screen deferred.** Present in the handoff (screen 3); out of v0-v1
  scope (no sensors). Build the other 5 screens; leave the humidity nav item /
  panel for the deferred slice.

## Responsive (from the handoff)

Desktop-first. ≤1040px: KPIs 2-col, dashboard sections stacked. ≤760px: sidebar →
60px icon rail (theme toggle hidden, follows OS), everything 1-col, spool table
horizontally scrollable. Dashboard stays fully legible (requirement). Reproduce
the proto's CSS-var breakpoints as real media queries.

## Out of scope (design)

No illustrations/imagery (the only "visual" is data colour). No drag-and-drop.
At most one simple modal. No custom charting beyond the 24h humidity sparkline
(deferred with humidity). No design work on the deferred humidity screen beyond
what the handoff already specifies.
# Cross-slice orchestration: spool auto-unload

When a web operation makes a spool Empty (remaining weight reaches zero) or
Archived, the app-crate handler calls `PrintersUseCases::unload_spool` after
the successful spool mutation. This is an intentional edge-orchestration seam:
the `spools` and `printers` domain slices remain independent, while the
driving adapter coordinates the two use cases. A database trigger was rejected
because it would hide this behaviour and make it harder to test. If more
cross-slice reactions accumulate, a domain-event/outbox design should replace
this explicit seam.

# AMS reconciliation panel (slice 23)

Voir glossaire § « AMS spool sync », ADR-0007, `docs/specs/23-ams-spool-sync.md`.
Handoff designer de référence : `init_assets/design_handoff_ams_reconciliation/`
(README + `Filature.dc.html` + captures).

**Décision : drawer ancré à droite, `position:fixed` hors du conteneur multicol.**
Remplace la première direction retenue (« panneau in-place ») : celle-ci s'est
révélée incompatible avec le masonry `.printer-grid{columns:340px}` — un panneau
in-place volumineux fragmente la carte (footer chevauchant la carte suivante), et
le contournement `column-span:all` fait **sauter toute la grille** (« ça fait
bouger tout l'écran », rejeté). Le drawer, `position:fixed` sibling de la grille
(pas un descendant), ne touche jamais au multicol → **zéro reflow**, de 1 à 16
bacs (4 unités AMS). Écartés aussi : vue dédiée pleine page (rompt le « in-place »
+ navigation pour un geste fréquent) ; popover ancré à la carte (déborde dès 2+
unités, JS de positionnement).

**Déclencheur & drawer.** Bouton « Synchroniser l'AMS » dans le header de carte,
visible seulement pour les imprimantes Bambu (groupe AMS) ; désactivé + tooltip si
injoignable (MQTT down). Clic → `hx-get` qui rend le drawer dans un conteneur
`#ams-drawer` **placé une seule fois hors de `.printer-grid`** (jamais dedans).
Drawer = scrim cliquable (`rgba(40,35,25,.28)`, = annuler) + panneau
`position:fixed` pleine hauteur à droite, largeur `min(400px,100vw)`,
`border-left:1px solid var(--border-strong)`, `box-shadow:-8px 0 24px rgba(40,35,25,.18)`.
Header (titre + `{{printer.name}} · {{n}} bacs` mono `--faint`), corps scrollable
groupé par **AMS Unit**, footer sticky.

**Badge d'état de synchro** sous le header de carte, à côté du badge d'occupation :
`à jour` (vert `--ok`/`--ok-bg`) · `désynchro` (ambre `--warn`, dès qu'un bac
diverge) · `hors ligne` (neutre `--faint`/`--active-bg`). Déclenchement 100%
manuel cette itération (pas de veille auto) : le badge reflète le **dernier état
connu** (état de synchro persisté par imprimante, mis à jour à chaque synchro).

**Cinq états de ligne** (une ligne par bac, omise si vide côté local ET machine) :

| État | Condition | Fond ligne | Badge | Select |
|---|---|---|---|---|
| **Match** | RFID lu = bobine du slot local | neutre | `RFID` vert | aucun (écart poids en info) |
| **Retiré** | bac vide machine, slot local chargé | ambre | `retiré` ambre | *Vider le slot* (défaut) / *Garder (ignorer)* |
| **Conflit** | RFID lu ≠ bobine du slot local | ambre | `conflit` ambre | bobine détectée RFID (défaut) / *Ignorer, garder local* |
| **Attribué** | pas de RFID, type+couleur matchent une chargeable | neutre | `attr.` ambre | select chargeable (15b), présélectionné |
| **Aucun** | pas de RFID, aucune correspondance | neutre | `aucun` neutre | select chargeable, vide |

Chip couleur = anneau `--border-strong` (hachure si transparent). Écart de poids
**toujours en lecture seule**, mono `--faint`, jamais de couleur/fond (poids
Filature autoritaire, ADR-0004). Footer : compteur à gauche (`« n à confirmer »`
ou `« Tout est synchronisé »` → bouton devient « Fermer »), Annuler (ghost) /
Confirmer (n) (accent). **Piège :** Confirmer applique le défaut présélectionné
même si l'utilisateur n'a pas touché le select (pas seulement les choix modifiés).

Server-rendered + htmx, pas de SPA/JS de positionnement. Aucun nouveau token.
i18n en+fr. Réutilise chip + gauge + select chargeable de `15b`.

**Slice 23b (raffinements).** Deux ajouts au drawer, sans nouvelle surface ni
modale (voir `docs/specs/23b-ams-reconciliation-refinements.md`) :
- **Match couleur tolérant** — le match `attr.` passe de l'hex exact à une distance
  perceptuelle CIELAB ΔE sous un seuil (matière toujours requise) ; le badge reste
  `attr.`. Réduit les faux « aucun ».
- **Action inline « aligner »** — sur une ligne de bac présentant un écart de poids,
  un bouton discret (styles de contrôle existants) aligne le restant Filature sur
  l'estimation AMS (`remain% × poids net`), en htmx inline re-rendant la ligne.
  Opt-in strictement par bac ; l'écart reste sinon en lecture seule. Exception
  opérateur-initiée à ADR-0004 (voir ADR-0007).
