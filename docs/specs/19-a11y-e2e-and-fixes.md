> ⚙️ Brief généré par IA (Claude Code, orchestration). L'implémentation revient à Codex. À relire avant délégation.

## Agent Brief

**Category:** feature
**Summary:** Mettre en place un harnais e2e d'accessibilité (Playwright + axe-core) qui échoue sur les écarts a11y identifiés, puis corriger les écarts déterministes jusqu'au vert. Cycle rouge → fix → vert.

**Slice / context:**
Un audit RGAA/WCAG AA a été réalisé et documenté dans `docs/a11y-audit.md` (méthode : instance isolée, données `tools/demo-instance.json`, axe-core sur 8 écrans × 2 thèmes, contraste calculé, revue manuelle). Il liste des écarts hiérarchisés (bloquants B1–B3, majeurs M1–M5, mineurs m1–m5). Le repo n'a aujourd'hui **aucune** toolchain JS/e2e ; les tests existants sont en Rust (`crates/app/tests`) avec testcontainers. Le framework e2e a11y a été tranché : **Playwright + axe-core** (seul capable de couvrir le contraste et le DOM réel). Cette slice introduit ce harnais **et** ferme le cycle rouge→vert sur les findings déterministes.

**Desired behavior:**

*Partie 1 — Harnais e2e a11y (doit exister et tourner en local + CI).*
- Une suite Playwright s'authentifie via le gate de login réel, importe le jeu de démonstration (`tools/demo-instance.json`) dans une instance jetable, puis parcourt les écrans authentifiés (dashboard, liste bobines, détail bobine, assistant d'ajout, imprimantes, édition imprimante, matériaux, réglages, référentiels lieux/fabricants) en thème **clair et sombre**.
- Sur chaque écran, axe-core est exécuté avec les tags WCAG 2.1 A + AA. Les violations `critical` et `serious` **font échouer** la suite ; le détail (règle, nœuds, sélecteurs) est reporté lisiblement.
- L'instance de test est **isolée** (base de données et identifiants dédiés, port propre) et ne touche aucune donnée réelle ; elle est créée et détruite proprement par la suite (pas de conteneur ni volume résiduel — cf. la dette de fuite testcontainers connue du projet).
- Chaque finding de l'audit correspond à **un test nommé et traçable** (assertion ciblée quand elle est plus robuste que la règle axe générique : présence d'un nom accessible sur un contrôle donné, présence d'`aria-live` sur la zone de résultats, présence de `scope` sur les en-têtes, présence d'un `h1`, présence d'un lien d'évitement).
- Le **contraste** (findings M1/M5) est couvert par un test **advisory** : exécuté et reporté, mais **non bloquant** tant que la décision de palette (`faint`/`muted`) n'est pas prise. Il doit être trivial de le rendre bloquant plus tard (un seul flag/tag).

*Partie 2 — Corrections déterministes (la suite doit passer au vert dessus).*
Rendre vrai, pour chaque finding déterministe, le comportement attendu décrit dans `docs/a11y-audit.md` :
- **B1** — tout champ de formulaire éditable (y compris les champs *inline* des tables référentielles matériaux / lieux / fabricants, et le sélecteur natif de couleur) expose un nom accessible traduit.
- **B2** — le sélecteur de sensibilité matériau expose un nom accessible traduit.
- **B3** — le bouton icône « décharger un slot » (et le lien d'édition imprimante icône) expose un nom accessible traduit.
- **M2** — après filtrage/recherche sur la liste des bobines, le nombre de résultats est annoncé aux technologies d'assistance (live-region), en réutilisant le motif de live-region déjà présent sur les messages de formulaire.
- **M3** — la page de détail d'une bobine possède un `h1` (l'identité de la bobine), sans régression visuelle.
- **M4** — tous les en-têtes de colonne des tables de données portent `scope="col"`.
- **m1** — un lien d'évitement « aller au contenu » est présent en tête de page et cible la zone de contenu principale.

**Edge cases / contraintes :**
- Les textes des noms accessibles passent par le système i18n existant (clés de traduction, FR + EN) — pas de chaîne en dur.
- Aucune régression visuelle sur les thèmes clair/sombre ni sur le rendu htmx (les swaps de contenu conservent les attributs a11y : les partiels re-rendus doivent rester conformes, pas seulement le premier rendu).
- Le lien d'évitement doit être exploitable au clavier et visible au focus.

**Key interfaces:**
- Templates Tera des écrans et partiels concernés (tables référentielles, chargement slots, détail bobine, filtre bobines, layout de base) — le harnais explore le repo à jour, pas de chemins figés ici : voir `docs/a11y-audit.md` pour la localisation actuelle de chaque écart.
- Système i18n du projet (clés de traduction FR/EN) pour tout nouveau libellé.
- Endpoints d'authentification (`POST /login`) et d'import (`POST /settings/import`, multipart `backup` + `confirm_replace`) — utilisés par la suite pour préparer l'état.
- Nouvelle toolchain e2e : `package.json`, config Playwright, `@axe-core/playwright` (ou injection d'`axe-core`), scripts d'exécution.

**Acceptance criteria:**
- [ ] `npm run` (ou équivalent documenté) démarre l'instance jetable, exécute la suite a11y sur tous les écrans × 2 thèmes, puis nettoie sans conteneur/volume résiduel.
- [ ] Sur le code **actuel** (avant corrections), la suite **échoue** avec au moins les règles `label`, `select-name`, `page-has-heading-one`, et les assertions ciblées scope/aria-live/skip-link (preuve du rouge initial, à consigner dans la description de PR).
- [ ] Après corrections, la suite **passe au vert** sur tous les findings déterministes (B1, B2, B3, M2, M3, M4, m1), dans les deux thèmes.
- [ ] Le test de contraste est présent, s'exécute, reporte les violations, et est **advisory** (non bloquant) — commutable en bloquant par un seul changement documenté.
- [ ] Tous les nouveaux libellés a11y existent en FR et EN via l'i18n ; aucune chaîne en dur.
- [ ] Les partiels htmx re-rendus (édition inline, chargement de slot, lignes de table) restent conformes après swap.
- [ ] La suite est câblée en CI en mode **advisory d'abord** (cf. skill `ci-setup`), documentée dans le README/CONTRIBUTING pour l'exécution locale.
- [ ] La validation finale est faite via le build/run Docker du projet (gate de validation habituel), pas seulement en local.

**Out of scope:**
- Décision et implémentation de la **palette de contraste** (M1/M5) : bloquée sur un arbitrage design (fusion `faint`→`muted` ou deux niveaux de gris). Le test contraste reste advisory ; aucune modification de token dans cette slice.
- Findings mineurs m2 (noms via `title`), m3 (sémantique `listbox` du slot-picker), m4 (`prefers-reduced-motion`) — hors première itération, à planifier ensuite.
- Refonte du système de tests Rust existant ; migration d'autres tests.
- Toute nouvelle fonctionnalité produit.

**References:**
- Audit a11y (findings, coordonnées actuelles, ratios, correctifs proposés) : `docs/a11y-audit.md`
- Product brief : `docs/product/brief.md`
- Glossaire : `docs/glossary.md`
- Design : `docs/design.md`
- ADRs : `docs/adr/`
- CI : skill `ci-setup` + `.harness/config.yml`
