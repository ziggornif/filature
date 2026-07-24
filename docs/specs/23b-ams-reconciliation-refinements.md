# 23b — Réconciliation AMS : match couleur tolérant + alignement du poids

> Brief généré par IA (harness, Claude Code) à partir de la discovery du
> 2026-07-24. Deux raffinements de la réconciliation AMS (slice 23), notés hors
> scope à l'époque. S'appuie entièrement sur 23 (drawer, matcher, états).

## Agent Brief

**Category:** feature
**Summary:** Rendre le match par attributs **tolérant aux couleurs proches** (distance
CIELAB ΔE au lieu de l'hex exact) et permettre à l'opérateur d'**aligner** le poids
restant Filature sur l'estimation `remain` de l'AMS, via une action inline par bac
dans le drawer de réconciliation.

**Slice / context:**
Étend la slice 23 (`docs/specs/23-ams-spool-sync.md`), déjà livrée : drawer de
réconciliation, cinq états (Match/Retiré/Conflit/Attribué/Aucun), matcher
comparant les bacs AMS aux slots chargés. Deux limites connues :
1. Le match par attributs exige une **couleur hex exacte** → la couleur réelle
   remontée par l'AMS ≠ rarement pile l'hex d'une bobine du stock → beaucoup de
   bacs classés « Aucun » alors que la bobine est visiblement la bonne.
2. Le drawer **affiche** l'écart de poids (AMS `remain` % vs restant Filature) mais
   en lecture seule ; aucun moyen d'aligner quand l'opérateur le juge pertinent.

**Desired behavior:**

### A. Match couleur tolérant (ΔE CIELAB)
- Dans la classification des états (matcher de 23), le critère couleur du match
  **Attribué** passe de « hex identique » à « **distance CIELAB ΔE ≤ seuil** ». La
  **matière reste requise à l'identique** (type filament == `material_name`).
- Parmi les bobines chargeables dont la matière correspond, on suggère la **plus
  proche en couleur** (ΔE minimal) **si elle est sous le seuil** ; sinon → **Aucun**
  (rien d'assez proche). Une bobine n'est suggérée qu'une fois (règle 23 inchangée).
- Le seuil est une **constante documentée et unique** (proposer ΔE2000 ≈ **10**, à
  confirmer/ajuster ; commenter le choix). ΔE2000 de préférence (perceptuellement
  correct) ; ΔE76 acceptable si ΔE2000 est trop lourd — documenter le choix retenu.
- La conversion hex→Lab et le calcul ΔE sont une **fonction pure testable** (util
  couleur, sans I/O). Réutiliser une crate éprouvée si elle existe déjà dans l'arbre,
  sinon implémentation minimale (sRGB→XYZ→Lab) — pas de dépendance lourde nouvelle
  sans raison.
- Le badge reste **`attr.`** (toujours un match par attributs). L'écart de poids et
  le reste de la ligne sont inchangés.
- **RFID inchangé** : la priorité RFID et les états Match/Retiré/Conflit ne sont pas
  touchés (ils ne dépendent pas de la couleur).

### B. Alignement du poids (action inline, opt-in)
- Sur une ligne de bac où **un écart de poids existe** (AMS `remain` % connu et
  différent du restant Filature de la bobine concernée), le drawer affiche, à côté
  de l'écart lecture seule, une **petite action « aligner »**.
- Cliquer « aligner » met le **poids restant Filature** de cette bobine à
  `round(remain% × poids_net)` (l'estimation AMS). C'est un **htmx inline** qui
  re-rend la ligne (ou le fragment concerné) : l'écart retombe à ~0, l'action
  disparaît. **Opt-in strictement par bac** — jamais automatique, jamais groupé
  avec « Confirmer ».
- L'alignement est **indépendant** de la confirmation de chargement : il agit sur le
  poids de la bobine, pas sur le lien slot↔bobine. Il s'applique à la bobine
  **actuellement associée à la ligne** (le slot chargé pour Match/Conflit/Retiré, ou
  la bobine suggérée/sélectionnée pour Attribué/Aucun — préciser : n'activer
  l'alignement que quand une bobine cible est déterminée et a un écart).
- Réutilise l'opération existante de mise à jour du **poids restant** d'une bobine
  (slice `03b`, « saisie du restant ») ; si le poids atteint 0, les règles de statut
  existantes s'appliquent (Empty + auto-unload de 15b) — comportement hérité, pas
  redéfini ici.
- Couture **cross-slice** dans le crate `app` (le drawer vit côté `printers`/app,
  le poids côté `spools`), comme l'auto-unload 15b et la réconciliation 23 :
  `printers` n'importe pas `spools` et inversement.

**Key interfaces:** (glossaire + API/SPI — pas de chemins)
- **Util couleur** (fonction pure, crate `app` ou util partagé) : `color_delta_e(hex_a,
  hex_b) -> f64` (ou équivalent), + un seuil constant `AMS_COLOUR_MATCH_MAX_DELTA_E`.
- **Matcher de réconciliation** (app crate, de 23) : le critère couleur du cas
  Attribué utilise `color_delta_e ≤ seuil` et choisit le **ΔE minimal** ; signature
  publique inchangée (mêmes états, mêmes rows).
- **Use case spools de mise à jour du restant** (API `spools`, existant `03b`) :
  réutilisé pour l'alignement ; pas de nouvelle capacité domaine si l'existante
  suffit (sinon, l'exposer proprement).
- **Handler web d'alignement** (app crate) : `POST` inline (htmx) qui prend
  (printer, bac/slot ou spool_id ciblé), calcule `remain% × net`, appelle le use case
  spools, re-rend le fragment de ligne. Erreurs : bobine/bac inconnu → 404 ; pas
  d'écart → no-op.

**Acceptance criteria (done contract):**
- [ ] Util couleur : tests de `color_delta_e` (mêmes couleurs → 0 ; paires proches <
      seuil ; paires éloignées > seuil ; hex court/`#`/casse tolérés comme ailleurs).
- [ ] Matcher : un bac PLA couleur **proche mais non identique** d'une bobine PLA
      chargeable → **Attribué** (badge `attr.`), la plus proche en ΔE ; un bac dont
      aucune bobine matière+couleur n'est sous le seuil → **Aucun** ; matière
      différente → jamais matché même si couleur proche ; RFID/Match/Retiré/Conflit
      inchangés (tests de non-régression 23 verts).
- [ ] Alignement : cliquer « aligner » sur une ligne avec écart met le restant
      Filature à `round(remain% × net)`, re-rend la ligne (écart ~0, action partie) ;
      opt-in par bac, jamais déclenché par « Confirmer » ; si le calcul donne 0 →
      statut Empty + auto-unload (hérité 15b) ; bobine/bac inconnu → 404 ; pas d'écart
      → pas d'action affichée.
- [ ] Couture cross-slice via app crate (assert : pas d'import `printers`↔`spools`) ;
      isolation + pureté domaine + capteurs hexagonaux verts.
- [ ] i18n en+fr : libellé « aligner », éventuel tooltip, tout nouveau texte.
- [ ] Suite complète verte, build offline + clippy propres, cache `.sqlx` à jour.

**Out of scope (YAGNI):**
- Alignement **groupé**/automatique ou depuis « Confirmer » (strictement inline, par
  bac, opt-in).
- Écriture du poids ailleurs que via l'action explicite (le `remain` AMS ne modifie
  jamais silencieusement le poids — ADR-0004 tient partout ailleurs).
- Réglage utilisateur du seuil ΔE (constante en dur, documentée ; pas d'UI de config).
- Toute modification des états RFID (Match/Retiré/Conflit), du drawer hors la ligne,
  du placement (drawer inchangé), ou de la logique de chargement 15b.
- Match couleur pour autre chose que la réconciliation AMS.

**Design (docs/design.md § AMS reconciliation panel) :** ajouter la mention de
l'**action inline « aligner »** sur les lignes à écart (bouton discret, réutilise les
styles de contrôle existants ; l'écart reste affiché en lecture seule tant qu'on
n'aligne pas). Pas de nouvelle surface ni de modale.

**References:**
- Slice amont : `docs/specs/23-ams-spool-sync.md` ; poids/restant : `docs/specs/03b-spools-ops.md`
- ADRs : `docs/adr/0007-ams-reconciliation.md` (addendum à compléter : alignement =
  exception opt-in, opérateur-initiée, à ADR-0004), `docs/adr/0004-net-weight-no-tare.md`
- Glossaire : `docs/glossary.md` (§ AMS spool sync)
- Design : `docs/design.md` (§ AMS reconciliation panel)
- Délégation Codex : mémoire `codex-delegation-workflow`
