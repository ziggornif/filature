> ⚙️ Brief généré par IA (Claude Code, orchestration). Implémentation : Codex. À relire avant délégation.

## Agent Brief

**Category:** feature
**Summary:** Amener le contraste au niveau AA (RGAA 3.2 / WCAG 1.4.3 + 1.4.11) en fusionnant le 3e niveau de gris dans `muted`, en corrigeant les pilules de statut et l'anneau de focus, puis basculer le test e2e de contraste en bloquant. Cycle rouge → vert.

**Slice / context:**
L'audit `docs/a11y-audit.md` (findings M1 contraste texte, M5 bordures/focus) reste ouvert. Le harnais e2e a11y (slice 19, mergé) contient déjà un test contraste **advisory** (`color-contrast`), non bloquant, activable par `A11Y_ENFORCE_CONTRAST=1`. Cette slice corrige les couleurs et rend ce test bloquant. **Décision de design déjà prise par le propriétaire : option A1 — on abandonne le 3e niveau de gris.**

**Desired behavior:**

*Palette (les valeurs cibles AA sont dans `docs/a11y-audit.md` ; les recalculer/valider, ne pas les copier aveuglément).*
- **A1 — fusion du gris pâle** : tout texte **porteur d'information** actuellement en `faint` (légendes de panneaux, en-têtes de tables, sous-lignes de slots, compteurs « g / g », pied de barre latérale, notes, etc.) passe en `muted` (qui satisfait AA sur tous les fonds). Le token `faint` ne subsiste que pour de l'ornement strictement non informatif (séparateurs, texte purement décoratif) — sinon il disparaît. Décision : auditer chaque usage de `faint` et le reclasser.
- **Pilules de statut** : les couleurs `ok` / `warn` / `danger` sur leur fond respectif (`ok-bg` / `warn-bg` / `danger-bg`) atteignent 4.5:1 dans **les deux thèmes**. Idem pour `danger` sur `bg`/`surface` (texte « bientôt vide ») et `muted` sur `active-bg` (pilule « ouverte »).
- **B2 — focus** : l'indicateur de focus clavier est visible à ≥ 3:1 sur son fond, dans les deux thèmes (l'anneau `accent` actuel échoue en sombre à 2.98:1). Les bordures de champ peuvent rester discrètes si le focus, lui, est franc — préférer renforcer le focus plutôt qu'épaissir toutes les bordures.
- **Cohérence** : la page de connexion (`login.html`, qui duplique un sous-ensemble de tokens en inline) reçoit les mêmes corrections — pas de régression de contraste sur l'écran de login.
- **Parité thèmes** : chaque correction tient en clair **et** en sombre.

*Test / garde-fou.*
- Le test e2e de contraste devient **bloquant par défaut** (plus advisory) : `color-contrast` critical/serious = échec de la suite. Le mécanisme de bascule existant (`A11Y_ENFORCE_CONTRAST`) est soit retiré, soit inversé (bloquant par défaut), de façon documentée.

**Edge cases / contraintes :**
- Ne pas dégrader la lisibilité ni l'intention « warm-neutral instrument » de la maquette : les corrections restent des ajustements de valeur, pas une refonte de palette. Les teintes (hue) sont conservées, seule la luminance bouge.
- Les swaps htmx (partiels re-rendus) doivent rester conformes — le test balaie déjà les écrans re-rendus.
- Aucune chaîne ni couleur codée en dur hors du système de tokens : tout passe par les variables CSS existantes.

**Key interfaces:**
- Tokens de design (variables CSS `--faint`, `--muted`, `--ok`, `--warn`, `--danger`, fonds associés, anneau de focus) — définis pour les deux thèmes (media query + attribut `data-theme`). Localisation actuelle : voir la feuille de styles principale et l'inline de la page login.
- Suite e2e a11y (slice 19) : le test `color-contrast` et son flag d'activation.
- Système i18n : inchangé (pas de nouveau libellé attendu).

**Acceptance criteria:**
- [ ] Sur le code **actuel**, activer le contraste bloquant fait **échouer** la suite (preuve du rouge — à consigner dans la PR).
- [ ] Après corrections, la suite e2e passe **entièrement au vert avec le contraste bloquant**, dans les deux thèmes, sur tous les écrans (y compris après swaps htmx).
- [ ] Aucun usage de `faint` ne porte plus d'information : soit reclassé en `muted`, soit strictement décoratif justifié.
- [ ] Pilules `ok`/`warn`/`danger`, texte « bientôt vide » et pilule « ouverte » ≥ 4.5:1 dans les deux thèmes (vérifiable par calcul).
- [ ] Indicateur de focus clavier ≥ 3:1 dans les deux thèmes.
- [ ] Page de connexion corrigée à l'identique (pas de token divergent en échec).
- [ ] `cargo fmt` propre ; les tests Rust existants restent verts (les assertions Rust qui matchent des couleurs de tokens, s'il y en a, sont mises à jour).

**Out of scope:**
- Mineurs m2 (noms via `title`), m3 (sémantique listbox du slot-picker), m4 (`prefers-reduced-motion`) — itération séparée.
- Toute refonte de la palette au-delà des ajustements de luminance nécessaires à l'AA.
- Nouvelles fonctionnalités produit ; modifications de layout (ex. sidebar) — traitées ailleurs.
- Câblage CI (skill `ci-setup`) — séparé.

**References:**
- Audit a11y (findings M1/M5, ratios mesurés, valeurs cibles calculées) : `docs/a11y-audit.md`
- Harnais e2e a11y : `e2e/` (test `color-contrast`, flag `A11Y_ENFORCE_CONTRAST`)
- Glossaire : `docs/glossary.md` · Design : `docs/design.md` · ADRs : `docs/adr/`
