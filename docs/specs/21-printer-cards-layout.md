# 21 — Cartes imprimantes : masonry, chips fluides, badge d'occupation

> Brief généré par IA (harness) à partir du brief design
> `init_assets/design_handoff_filature/BRIEF_imprimantes_3d.md`, relu par un humain.

## Agent Brief

**Category:** feature
**Summary:** Refondre le layout des cartes de l'écran Imprimantes : empilement masonry sans étirement, chips de Slot fluides, badge d'occupation `{chargés}/{total}` dans le header de carte, label de groupe systématique.

**Slice / context:**
S'appuie sur les slices Printers (`15a-printers-core`, `15b-printer-spool-loading`, `16-printer-heads`, `17-printer-ams-topology`). Aujourd'hui l'écran Imprimantes affiche une grille CSS stricte : chaque carte s'étire à la hauteur de la plus haute de sa ligne (grande zone vide en bas des cartes mono-bobine à côté d'une carte AMS), et les chips de Slot ont une largeur fixe qui laisse une bande vide à droite dans les cartes larges. Le header de carte n'indique pas le taux d'occupation, et le label de groupe n'apparaît sur un groupe mono-slot que quand le Slot est vide.

**Desired behavior:**

1. **Masonry** — le conteneur des cartes imprimantes empile en CSS multi-colonnes (`columns:340px; column-gap:16px` ; cartes `break-inside:avoid; margin-bottom:16px`). Chaque carte garde sa hauteur naturelle ; aucune zone vide interne due à un étirement de ligne. Pas de grid avec stretch implicite.
2. **Chips de Slot fluides** — les chips de bobine chargée et de Slot vide (groupes multi-slots, AMS/MMU) passent de largeur fixe à `flex:1 1 140px; min-width:140px` : ils occupent toute la largeur disponible de la carte quelle que soit sa largeur réelle.
3. **Badge d'occupation** — le header de chaque carte affiche une pastille `{chargés} / {total} chargées` entre le titre et le bouton éditer. `chargés` = nombre de Slots avec Loaded Spool, `total` = nombre total de Slots de l'imprimante, **calculés côté serveur** (dans le view model du fragment, comme les autres dérivés). Couleur sémantique :
   - tout chargé → `--ok` sur `--ok-bg`
   - partiellement chargé → `--warn` sur `--warn-bg`
   - tout vide → `--muted` sur `--active-bg` — ⚠️ la maquette utilise `--faint`, supprimé par la décision A1 (spec 20 / a11y) : **ne pas réintroduire**, utiliser `--muted`.
   - Style : pill mono ~10.5px, `padding:4px 9px`, `border-radius:100px` (cf. maquette).
   - Le libellé passe par une clé i18n (nouvelle clé, même mécanisme `t(key=…)` que le reste de l'écran).
4. **Label de groupe systématique** — les groupes mono-slot (« Bobine externe », « Bobine ») affichent leur label au-dessus du Slot dans tous les états (chargé comme vide), avec le même style de titre de groupe que les groupes multi-slots. Le libellé inline « {label} — Vide » du Slot vide mono-slot disparaît au profit du titre au-dessus + état « Vide » seul.
5. **Fragment htmx inchangé** — charger/décharger une bobine re-rend le fragment comme aujourd'hui ; le badge d'occupation reflète le nouvel état après swap sans rechargement de page.

**Key interfaces:** (glossaire : Printer, Print Head, AMS Unit, Filament Module, Slot, Loaded Spool)
- View model du fragment « printer loading » — chaque Printer y expose en plus `filled` / `total` (dérivés serveur, même veine que `loaded_spools_count`).
- Aucune évolution de domaine, de persistance ni de routes : dérivé d'affichage uniquement.

**Acceptance criteria:**
- [ ] Deux cartes de hauteurs très différentes (ex. AMS 4 slots vs mono-bobine) côte à côte : la carte courte garde sa hauteur naturelle (pas d'étirement, pas de zone vide interne).
- [ ] Dans une carte plus large que ~2×140px, les chips d'un groupe multi-slots occupent toute la largeur (pas de bande vide à droite du dernier chip).
- [ ] Badge header : imprimante 4/4 → pill verte « 4 / 4 chargées » ; 2/4 → ambre ; 0/4 → neutre (`--muted`, jamais `--faint`).
- [ ] `filled`/`total` calculés côté serveur — aucun JS de comptage côté client.
- [ ] Charger puis décharger une bobine via le fragment htmx met à jour le badge sans rechargement complet.
- [ ] Groupe mono-slot chargé : label de groupe visible au-dessus du chip, même style que les titres de groupes AMS.
- [ ] Libellé du badge i18n (pas de chaîne en dur dans le template).
- [ ] Thèmes clair et sombre : badge lisible dans les trois états (tokens sémantiques existants).
- [ ] e2e a11y (Playwright + axe) : écran Imprimantes reste sans violation ; test de contraste bloquant vert.
- [ ] Suite de tests complète verte.

**Out of scope:**
- Passage px → rem : **décision actée de rester en px** (voir `docs/design.md` § unités) — ne rien convertir.
- Toute évolution du domaine printers (topologie, heads, feed modes) ou des routes.
- Les autres écrans (Bobines, Dashboard…) et leurs grilles.
- Réintroduction du token `--faint` sous quelque forme que ce soit.
- Drag-and-drop, réordonnancement de cartes, préférences de tri.

**References:**
- Brief design source : `init_assets/design_handoff_filature/BRIEF_imprimantes_3d.md`
- Maquette hifi : `init_assets/design_handoff_filature/Filature.dc.html` (écran Imprimantes)
- Design : `docs/design.md` · Glossaire : `docs/glossary.md`
- Décision A1 (suppression `--faint`) : `docs/specs/20-a11y-contrast-palette.md`
- Specs amont : `docs/specs/15a…17-printer-*.md`
