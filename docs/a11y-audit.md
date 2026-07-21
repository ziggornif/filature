# Audit d'accessibilité — Filature

**Cible** : RGAA 4.1 / WCAG 2.1 niveau **AA**
**Date** : 2026-07-17
**Périmètre** : interface web authentifiée (dashboard, bobines, détail bobine, assistant d'ajout, imprimantes, matériaux, réglages, référentiels lieux/fabricants, connexion), thèmes clair **et** sombre.

## Méthode

Double capteur, croisé avec une revue manuelle :

1. **Instance d'audit isolée** — image docker dédiée (`-p filature-a11y`, port 8081, volume séparé), jeu de données `tools/demo-instance.json` importé (18 bobines, imprimantes, matériaux). Aucune donnée réelle touchée.
2. **axe-core** (WCAG 2.1 A+AA + best-practice) exécuté par Playwright sur **8 écrans × 2 thèmes = 16 scans** (+ formulaires/référentiels), après authentification réelle.
3. **Contraste calculé depuis les design tokens** (`app.css`) — ratios WCAG exacts pour les deux thèmes, indépendamment du rendu.
4. **Revue manuelle des templates Tera + JS** — sémantique, landmarks, hiérarchie de titres, contenu dynamique htmx, clavier, alternatives textuelles.

Scripts et résultats bruts : `axe-results.json`, `contrast.js` (scratchpad de session, non versionnés).

## Synthèse

| Gravité | Nombre | Nature |
|---|---|---|
| 🔴 Bloquant | 3 | Champs sans étiquette, `select` sans nom, bouton icône sans nom |
| 🟠 Majeur | 5 | Contraste texte (388 occurrences axe), résultats de filtre non annoncés, page détail sans `h1`, `th` sans `scope`, contraste bordures/focus |
| 🟡 Mineur | 5 | Lien d'évitement absent, noms via `title` seul, dropdown custom, `prefers-reduced-motion`, dette tokens page login |

**Verdict** : socle sérieux (langue dynamique, landmarks, live-regions sur les erreurs de formulaire, icônes décoratives correctement masquées, aucun `<img>` sans alternative), mais **non conforme AA** aujourd'hui. Les deux blocages structurels (étiquettes de formulaire, contraste) sont systémiques mais corrigeables par des changements localisés + un ajustement de la palette.

---

## 🔴 Bloquant

### B1 — Champs de formulaire sans étiquette (RGAA 11.1 / WCAG 4.1.2, 3.3.2)
axe : **`label` — critical, 34 nœuds** (materials, settings-locations, spool-edit, les deux thèmes).

Les champs éditables *inline* des tables référentielles n'ont aucune étiquette programmatique — l'en-tête de colonne ne fait pas office d'étiquette pour axe/AT.

- `_material_row.html:3,8,15,20,39,45` — `name`, `density`, `drying_temp_c`, `drying_time_h`, `nozzle_c`, `bed_c`
- `_location_row.html:3,8` — `name`, `note`
- `_manufacturer_row.html` — mêmes champs `name`/`country`
- `_spool_wizard_details.html:65` — `<input type="color" class="colour-picker">` (le `<label class="colour-preview">` qui l'enveloppe n'a pas de texte)

**Correctif** : ajouter un `aria-label` (traduit) sur chaque champ, p.ex.
`<input name="density" aria-label="{{ t(key='materials.col.density') }} — {{ m.name }}">`.
Pour le sélecteur de couleur : `aria-label` sur l'`<input type="color">` (le champ hex voisin en a déjà un, ligne 69).

### B2 — `select` sans nom accessible (RGAA 11.1 / WCAG 4.1.2)
axe : **`select-name` — critical, 2 nœuds** (materials, 2 thèmes).

`_material_row.html:28` — `<select name="sensitivity">` (pilule de sensibilité) sans nom.

**Correctif** : `aria-label="{{ t(key='materials.col.sensitivity') }} — {{ m.name }}"`.

### B3 — Bouton icône « décharger » sans nom accessible (RGAA 11.9 / WCAG 4.1.2)
`_printer_loading.html:23` — `<button type="submit" title="…">✕</button>`. Le nom accessible calculé est le caractère « ✕ » ; `title` n'est pas un nom fiable pour un bouton.

**Correctif** : `aria-label="{{ t(key='printers.slot.unload') }}"` sur le bouton, garder le `title` pour l'infobulle. (Le lien édition `✎` ligne 6 a le même défaut — voir M-mineur.)

---

## 🟠 Majeur

### M1 — Contraste de texte insuffisant (RGAA 3.2 / WCAG 1.4.3)
axe : **`color-contrast` — serious, 388 nœuds** répartis sur tous les écrans, l'écran **imprimantes** en tête (89 nœuds/thème). Confirmé par le calcul depuis les tokens.

Le token **`--faint`** échoue le 4.5:1 sur **tous** les fonds, dans **les deux** thèmes — or il porte du texte informatif partout (légendes de panneaux, `<th>` de tables, sous-lignes de slots, pied de barre latérale, notes). Les pilules de statut (`--ok`, `--warn`, `--danger` sur leur fond) sont limites/en échec selon le thème.

Ratios mesurés (sélection) :

| Paire | Sombre | Clair | Requis |
|---|---|---|---|
| `faint` / `raised` | **2.64** | **3.04** | 4.5 |
| `faint` / `surface` | **2.94** | **2.77** | 4.5 |
| `ok` / `ok-bg` (pilule) | **4.47** | **4.17** | 4.5 |
| `warn` / `warn-bg` | 4.74 | **3.61** | 4.5 |
| `danger` / `danger-bg` | **3.58** | 4.66 | 4.5 |
| `danger` / `surface` | **3.89** | 5.36 | 4.5 |
| `muted` / `active-bg` (pilule ouverte) | 4.89 | **4.19** | 4.5 |

**Correctif — palette (valeurs calculées passant AA, à revalider avec `contrast.js`)** :

| Token | Actuel (sombre → clair) | Proposé (sombre → clair) |
|---|---|---|
| `--faint` | `#736c60` → `#9b9284` | `#9a9387` → `#6e6557` |
| `--ok` | `#5aa86b` → `#3f7d4e` | `#5dab6e` → `#397748` |
| `--warn` | `#c9922f` → `#a06f1c` | *(sombre OK)* → `#8e5d0a` |
| `--danger` | `#d6584a` → `#b23a2e` | `#eb6d5f` → *(clair OK)* |

⚠️ En clair, `--faint` conforme (`#6e6557`) devient quasi identique à `--muted` (`#6d675c`) : la hiérarchie à trois niveaux de gris n'est pas soutenable en AA. **Décision de design à prendre** : soit fusionner `faint`→`muted` pour le texte porteur d'information et ne garder `faint` que pour du décoratif, soit accepter deux niveaux seulement.

### M2 — Résultats du filtre bobines non annoncés (RGAA 7.4 / WCAG 4.1.3)
`spools.html:41-43,126` — le formulaire de filtre fait `hx-get /spools/rows` → `hx-swap` dans `#spools-table-body`, et `#spools-filtered-count` (ligne 13) se met à jour. **Aucune de ces zones n'est une live-region** : un lecteur d'écran n'annonce ni le nouveau nombre de résultats ni le rafraîchissement de la liste après filtrage/recherche.

**Correctif** : envelopper le compteur (ou une zone de statut visuellement discrète) d'un `aria-live="polite"` mis à jour à chaque swap, p.ex. `role="status"` sur `.spools-count` ou un `<span aria-live="polite">` dédié annonçant « N bobines affichées ». (Le motif live-region existe déjà pour `#materials-msg` / `#locations-msg` — le réutiliser.)

### M3 — Page détail bobine sans `h1` (RGAA 9.1 / WCAG 1.3.1, 2.4.6)
axe : **`page-has-heading-one` — moderate**, `spool-detail@light` + `@dark`.
`_spool_detail_card.html:22-23` — l'identité de la bobine est un `<strong>`, pas un titre. La page n'a aucun `h1`.

**Correctif** : passer le nom de la bobine en `<h1>` (stylé pour conserver le rendu), ou ajouter un `h1` visuellement intégré à l'en-tête.

### M4 — En-têtes de tableaux sans `scope` (RGAA 5.7 / WCAG 1.3.1)
**0 des 28 `<th>`** portent `scope`. Concerne les tables de données `spools`, `materials`, `locations`, `manufacturers`.

**Correctif** : `scope="col"` sur chaque `<th>` d'en-tête de colonne (`thead th`). Trivial et sans impact visuel.

### M5 — Contraste des bordures de composants et de l'anneau de focus (RGAA 3.2 / WCAG 1.4.11)
Calcul depuis tokens :

| Élément | Sombre | Clair | Requis |
|---|---|---|---|
| `border-strong` / `raised` (bordure d'input) | **1.44** | **1.68** | 3.0 |
| `border` / `surface` | **1.25** | **1.23** | 3.0 |
| Anneau de focus `accent` / `raised` (sombre) | **2.98** | 7.12 | 3.0 |

La bordure est souvent le **seul** indicateur du contour d'un champ (fond `raised`/`surface` proche du fond de page). En thème sombre l'anneau de focus des inputs (`box-shadow accent`) passe tout juste sous 3:1.

**Correctif** : `--border-strong` sombre → `#7d7870`, clair → `#928a7b` (3:1) ; **ou** conserver la palette et ajouter un contour de focus dédié à fort contraste (`outline: 2px solid var(--text)` sur `:focus-visible`). C'est un arbitrage design (bordures plus marquées vs. esthétique « instrument discret »).

---

## 🟡 Mineur / bonnes pratiques

- **m1 — Lien d'évitement absent (RGAA 12.7)** : `base.html` place la barre latérale/nav avant `<main>` sur chaque page, sans « aller au contenu ». Ajouter un skip-link en tête de `<body>` ciblant `#main`.
- **m2 — Noms accessibles via `title` seul (RGAA 11.9)** : bascule vue table/cartes (`spools.html:21,25`, radios nommés uniquement par une icône + `title`) et lien édition imprimante `✎` (`_printer_loading.html:6`). Ajouter des `aria-label` explicites.
- **m3 — Dropdown `slot-picker` custom (RGAA 7.1)** : `<details>/<summary>` + `<button>` (slot-picker.js). Clavier utilisable a minima (Tab + Échap gérés), mais pas de sémantique `listbox`/`option` ni navigation aux flèches. Acceptable, à surveiller si l'usage s'étend.
- **m4 — `prefers-reduced-motion` absent** : transitions `transform`/`scale` (presets couleur, cartes). Ajouter `@media (prefers-reduced-motion: reduce)` neutralisant les animations. (WCAG 2.3.3 est AAA, mais bonne pratique.)
- **m5 — Page login duplique les tokens** (`login.html`, styles inline car `/static` derrière le gate) : elle hérite de la même dette de contraste que la palette globale — appliquer les corrections M1/M5 aux deux endroits.

---

## ✅ Points déjà conformes (à préserver)

- `lang` dynamique (`base.html:2`) et attribut de thème — RGAA 8.
- Aucune image bitmap sans alternative ; toutes les icônes SVG décoratives portent `aria-hidden="true"` — RGAA 1.
- Live-regions présentes sur les messages d'erreur de formulaire (`role="alert" aria-live="polite"` sur `#materials-msg`, `#locations-msg`, `#settings-msg`) — RGAA 7.4.
- `autocomplete="username"` / `current-password` + `<label for>` corrects sur la connexion — RGAA 11.
- Landmarks `<aside>`/`<nav>`/`<main>` structurés ; `viewport` sans blocage du zoom — RGAA 12/10.
- `hx-confirm` natif sur les suppressions ; focus visible présent (outline UA sur boutons, anneau sur inputs).

---

## Prochaine étape — tests e2e a11y dédiés

Le script d'audit (Playwright + `@axe-core/playwright`) est directement industrialisable en garde-fou de non-régression :

- login → import `demo-instance.json` → itération routes × {clair, sombre} → assertion **0 violation critical/serious**.
- Cible d'abord les blocages B1–B3 + M3 (déterministes) ; le contraste (M1/M5) une fois la palette corrigée.
- À brancher en CI advisory d'abord (cf. `ci-setup`), bloquant ensuite.

Voir la conversation pour le harnais e2e à mettre en place.
