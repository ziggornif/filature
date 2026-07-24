# Handoff : Réconciliation AMS (slice 23) — placement du panneau

## Décision (remplace l'arbitrage ouvert dans le brief)
**Drawer ancré à droite, plein hauteur, position:fixed hors du conteneur multicol.** Ni modale centrée, ni vue dédiée pleine page, ni `column-span:all`.

Pourquoi : le drawer ne participe pas au flux `.printer-grid{columns:340px}` (il est `position:fixed`, sibling du conteneur, pas un descendant qui casse `break-inside:avoid`) → **zéro reflow du masonry**, quel que soit le nombre de bacs (1 à 16, jusqu'à 4 unités AMS). Contrairement à `column-span:all` (rejeté : "ça fait bouger tout l'écran"), rien ne bouge derrière le scrim. Contrairement à une vue dédiée (`/printers/{id}/ams-reconciliation`), pas d'aller-retour de navigation pour un geste qu'on veut fréquent et rapide — cohérent avec le "sans rechargement" déjà en place ailleurs dans l'app (filtres bobines, édition inline du poids).

Options écartées et pourquoi (documentées pour traçabilité ADR-0007) :
- **Vue dédiée pleine page** : la plus simple à raisonner côté Rust, mais rompt le principe "in-place" et ajoute une navigation pour une action courante.
- **Popover ancré à la carte** (position calculée près du bouton) : lisible pour 1 unité AMS (4 bacs), devient un pavé qui déborde ou nécessite un scroll interne serré dès 2+ unités (jusqu'à 16 bacs) ; le plus fragile en responsive/mobile ; nécessite du JS de positionnement (anti-pattern pour ce projet, "pas de SPA/JS lourd").

## Ce qui a changé côté maquette
Le prototype `Filature.dc.html` (référence visuelle, cf. règles ci-dessous) a été mis à jour :
1. Bouton **« Synchroniser l'AMS »** dans le header de carte, visible uniquement pour les imprimantes ayant un groupe `AMS` (Bambu). Désactivé + tooltip si l'imprimante est injoignable (MQTT down).
2. **Badge d'état de synchro au repos**, sous le header de carte, à côté du badge d'occupation existant : `à jour` (vert), `désynchro` (ambre, dès qu'au moins un bac diverge), `hors ligne` (neutre). Le déclenchement reste 100% manuel pour cette itération (pas de veille auto) — le badge reflète juste le dernier état connu.
3. **Drawer de réconciliation** : overlay `position:fixed` pleine hauteur à droite (largeur `min(400px,100vw)`), scrim cliquable = annuler, liste des bacs groupée par unité AMS, footer sticky Annuler/Confirmer(n).

## About the Design Files
`Filature.dc.html` est une **référence de design HTML** — prototype d'apparence/comportement, pas du code de production. Framework interne (`<x-dc>`, `<sc-for>`, `<sc-if>`, holes `{{ }}`) non pertinent pour l'implémentation.

**Cible** : HTML server-rendered + CSS vanilla + htmx, binaire Rust unique, sans framework JS front. Le drawer doit être un **fragment htmx** : le bouton `hx-get` le contenu du drawer dans un conteneur `#ams-drawer` placé une fois en dehors de `.printer-grid` (jamais à l'intérieur, pour ne jamais retoucher au multicol) ; swap `innerHTML` ou `outerHTML` de `#ams-drawer`. Confirmer = `hx-put`/`hx-post` qui renvoie soit le drawer fermé (vide), soit re-render les cartes imprimante affectées (`hx-swap-oob` sur les cartes concernées) + ferme le drawer.

## Fidelity
High-fidelity pour les tokens/layout (couleurs, typo, espacement identiques au reste de l'app — rien de nouveau introduit). Les données affichées (RFID, écarts de poids, candidats attribués) sont **simulées** dans le prototype ; le contenu réel vient du backend déjà fait (PR #98).

## Contenu du drawer

### Header
Titre « Synchroniser l'AMS » + `{{printer.name}} · {{n}} bacs` en mono, `--faint`.

### Corps — une ligne par bac, groupé par unité AMS (`AMS Unit {u}`, label mono uppercase `--faint`, comme les labels de groupe existants)
Chaque ligne : pastille couleur (anneau `--border-strong`, hachure si transparent) · titre (`ams{u}-{n}` + type + sous-marque) · écart poids (lecture seule, texte mono `--faint`, jamais de couleur/fond — poids Filature autoritaire, ADR-0004) · badge à droite · select conditionnel selon l'état :

| État | Condition (lecture MQTT vs local) | Fond de ligne | Badge | Select |
|---|---|---|---|---|
| **Match** | RFID lu = bobine locale du slot | neutre (`--surface`, bord `--border`) | `RFID` vert (`--ok`/`--ok-bg`) | aucun — juste l'écart de poids en info |
| **Retiré** | bac vide côté imprimante, slot local encore chargé | ambre (`--warn-bg`, bord `--warn`) | `retiré` ambre | 2 options : *Vider ce slot en local* (défaut) / *Garder tel quel (ignorer)* |
| **Conflit** | RFID lu ≠ bobine locale du slot | ambre (`--warn-bg`, bord `--warn`) | `conflit` ambre | 2 options : bobine détectée par RFID (défaut) / *Ignorer, garder local* |
| **Attribué** | pas de RFID, mais type+couleur (hex) matchent une bobine chargeable | neutre | `attr.` ambre (texte seul, pas de fond de ligne coloré) | select chargeable complet (réutilise le select de 15b), présélectionné sur le candidat |
| **Aucun** | pas de RFID, aucune correspondance type+couleur | neutre | `aucun` neutre (`--faint`/`--active-bg`) | select chargeable complet, vide par défaut |

Slots vides à la fois côté local ET côté imprimante : **omis** de la liste (rien à réconcilier).

### Footer
Compteur à gauche (`« n à confirmer »` ou `« Tout est synchronisé »`), boutons Annuler (ghost) / Confirmer (n) (accent) à droite. Confirmer applique, pour chaque ligne actionnable, la valeur sélectionnée (ou le défaut présélectionné si l'utilisateur n'a pas touché le select — **piège à éviter côté implémentation** : le défaut affiché doit être appliqué tel quel si l'utilisateur clique Confirmer sans interagir, pas seulement les choix explicitement modifiés).

## Design Tokens
Aucun nouveau token. Réutilise intégralement `design.md` (greys atelier, `--ok`/`--warn`/`--danger`, IBM Plex Sans/Mono, rayons 8-10px, `--shadow`). Le drawer : largeur `min(400px,100vw)`, `border-left:1px solid var(--border-strong)`, `box-shadow:-8px 0 24px rgba(40,35,25,.18)` (clair) — même ombre plus opaque en sombre comme le reste de l'app.

## Question connexe (hors scope de ce lot)
Tolérance colorimétrique pour le "attr." (actuellement hex exact) reste une amélioration produit séparée, non traitée ici — cf. brief original.

## Captures
- `screenshots/printer-grid-badges.png` — grille imprimantes, badge « désynchro » sur P1S Atelier.
- `screenshots/drawer-conflicts.png` — drawer ouvert sur P1S Atelier : ligne « retiré » (bac vidé côté imprimante) et ligne « aucun » (spool détecté sans correspondance), footer « 2 à confirmer ».
- `screenshots/drawer-ok-state.png` — drawer ouvert sur une imprimante entièrement synchronisée (X1C Bureau), footer « Tout est synchronisé ».

## Assets / Files
- `Filature.dc.html`, `support.js` — prototype à jour (référence visuelle uniquement), écran **Imprimantes 3D**.
- Réfs projet : `docs/specs/23-ams-spool-sync.md`, `docs/design.md` (§ AMS panel — à mettre à jour avec la décision "drawer" ci-dessus), `docs/adr/0007`.
