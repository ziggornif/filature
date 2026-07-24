# 23 — Synchro bobines AMS : réconciliation trays Bambu ↔ bobines Filature

> Brief généré par IA (harness, Claude Code) à partir de la discovery du
> 2026-07-24, relu par un humain. S'appuie sur le payload AMS déjà capturé au
> spike Bambu (#92) — technique dérisquée. Décisions figées dans **ADR-0007**.

## Agent Brief

**Category:** feature
**Summary:** Lire les bacs (Trays) d'un AMS Bambu via le proxy MQTT à la demande
(22b) et **réconcilier** chaque Tray avec une bobine Filature — suggestion RFID
puis attributs, confirmée par l'opérateur — pour charger les Slots AMS sans
double-saisie.

**Slice / context:**
Étend la feature Machine Link (`22a`/`22b`) et le chargement de bobines (`15b`).
Aujourd'hui l'opérateur met physiquement une bobine dans l'AMS **puis** la charge
à la main dans un Slot Filature (double-saisie). La topologie AMS Units / Slots
`ams{u}-{n}` existe (`17`), le chargement `load_slot` avec exclusivité +
statut-chargeable + auto-unload existe (`15b`), la connexion Bambu LAN MQTT et le
rapport `pushall` existent (`22b`). Le rapport `pushall` contient déjà
`print.ams.ams[].tray[]` (type, couleur, sous-marque, `remain`, `tag_uid`) mais
il est actuellement ignoré au parsing. Cette slice le lit et ajoute la
**Réconciliation AMS** : suggérer une bobine par Tray, l'op confirme/corrige,
la confirmation charge le Slot. **Bambu uniquement.**

**Desired behavior:**

1. **Lecture des Trays AMS (Bambu).** Un nouvel appel à la demande ouvre une
   session MQTT courte (même séquence connect → subscribe → `pushall` → rapport
   complet → disconnect que 22b) et retourne la liste des **AMS Trays** :
   pour chaque tray, l'index d'unité + l'index de bac, le type filament, la
   couleur (hex), la sous-marque, le `remain` (%), et le `tag_uid`. Un `tag_uid`
   « tout à zéro » (`0000…`) est normalisé en **absent** (bobine tierce). Le
   rapport peut être partiel : on n'agit que sur un rapport contenant la section
   `print.ams`. Timeout ≈ 3 s (comme 22b) → sinon erreur remontée à l'UI, aucun
   Slot modifié. Prusa / Moonraker n'exposent aucun Tray : l'appel y est
   indisponible (pas de bouton, ou erreur explicite si forcé).

2. **Suggestion de match (par Tray).** Pour chaque Tray, le système propose une
   bobine Filature :
   - **RFID d'abord** : si le `tag_uid` du Tray est présent et déjà mémorisé sur
     une bobine (**AMS Tag UID** = attribut bobine), match certain.
   - **Sinon attributs** : meilleure correspondance sur type filament ↔
     `material_name` et couleur ↔ hex (la sous-marque sert d'indice/affichage,
     pas de clé dure), **parmi les bobines chargeables** (Sealed/Open, non
     chargées ailleurs). Une bobine n'est suggérée qu'une fois par réconciliation.
   - **Aucun candidat** → pas de suggestion ; le Slot reste à charger à la main
     via le select `15b` habituel.

3. **Panneau de réconciliation.** Le panneau liste une ligne par Tray, rattachée
   au Slot AMS correspondant (clé `ams{u}-{n}`), montrant les attributs du Tray,
   la bobine suggérée (dans un select réutilisant la liste chargeable de `15b`,
   modifiable par l'op), et l'**écart de poids** en lecture seule (`remain` AMS
   vs restant Filature). L'op peut décharger/ignorer une ligne (aucun match).

4. **Confirmation.** Valider le panneau applique, par ligne confirmée :
   - **mémorise le `tag_uid`** sur la bobine choisie si le Tray porte un UID réel
     (jamais `0000…`), pour que la prochaine synchro soit un match certain ;
   - **charge le Slot** via `load_slot(printer, ams{u}-{n}, spool)` — hérite
     **sans changement** de l'exclusivité (une bobine dans au plus un Slot du
     parc, déplacement si déjà chargée ailleurs), du garde Sealed/Open, et de
     l'auto-unload Empty/Archived. Slot inconnu / printer inconnu → 404.
   - **ne touche jamais** le poids restant Filature (source = pesée, ADR-0004) ;
     l'écart est montré, pas appliqué (l'action d'alignement est hors scope — voir
     ci-dessous).

5. **Surface & garde-fous.** Bouton « Synchroniser l'AMS » sur la carte d'une
   Printer **Bambu** ayant un Machine Link **et** ≥ 1 AMS Unit (sinon absent).
   Feature soumise au même gate que Machine Link : **désactivée sur l'instance
   démo**. Aucun statut/Tray persisté hormis le `tag_uid` mémorisé sur la bobine.

6. **Mapping Tray → Slot.** L'ordre des unités `print.ams.ams[]` et des bacs
   `tray[]` correspond aux AMS Units ordonnées (`17`) et à leurs Slots
   `ams{u}-0..3`. Un Tray sans correspondance de Slot (config Filature plus
   petite que l'AMS physique) est signalé, non chargé.

**Key interfaces:** (glossaire + API/SPI — pas de chemins de fichiers)
- **`AmsTray`** (nouveau, domaine `printers::machine`) — `unit_index`,
  `tray_index`, `filament_type: Option<String>`, `color_hex: Option<String>`,
  `sub_brand: Option<String>`, `remain_percent: Option<u8>`,
  `tag_uid: Option<String>` (normalisé : `0000…` → `None`).
- **`MachineStatusProbe`** (SPI) — nouvelle capacité `fetch_ams(&MachineLink) ->
  Result<Vec<AmsTray>, MachineError>`. Bambu parse `print.ams.ams[].tray[]` dans
  le rapport `pushall` ; PrusaLink / Moonraker → indisponible (vide ou erreur
  dédiée). Le parsing du statut existant (`parse_complete_report`) reste inchangé.
- **`MachineConnectivityUseCases`** (API printers) — `fetch_ams_trays(PrinterId)
  -> Result<Vec<AmsTray>, MachineError>` (Bambu only ; sinon indisponible).
- **`Spool` / `NewSpool` / `EditSpool`** (domaine spools) — nouvel attribut
  **AMS Tag UID** `ams_tag_uid: Option<String>`. Use case spools pour mémoriser
  l'UID sur une bobine (`memorize_ams_tag(SpoolId, tag_uid)` ou équivalent),
  idempotent, ne stocke jamais un UID vide/`0000…`.
- **Requête bobines réconciliables** (SPI spools) — étend/complète la requête des
  bobines chargeables pour exposer au matcher : `material_name`, couleur hex,
  `ams_tag_uid`, statut. Slice isolation : champs d'affichage/match en primitifs,
  `printers` n'importe pas `spools` et inversement.
- **`PrintersUseCases::load_slot`** (API, inchangé) — réutilisé tel quel pour
  charger le Slot AMS à la confirmation.
- **Orchestration app-crate `AmsReconciliationService`** (la couture cross-slice,
  précédent `15b` + ADR-0007) — fetch trays (printers API) + fetch bobines
  réconciliables (spools) → **matcher pur** (RFID puis attributs) → rend le
  panneau → à la confirmation : mémorise l'UID (spools API) puis `load_slot`
  (printers API). Le matcher est une fonction pure testable, hors des deux
  domaines.

**Acceptance criteria (done contract):**
- [ ] Tests unitaires du matcher (fonction pure) : un Tray dont le `tag_uid` est
      mémorisé sur une bobine → cette bobine ; sinon meilleur match type+couleur
      parmi les chargeables ; une bobine déjà suggérée n'est pas re-proposée ;
      aucun candidat → pas de suggestion ; `tag_uid` `0000…` traité comme absent.
- [ ] Parsing Bambu : un rapport `pushall` avec `print.ams.ams[].tray[]` produit
      les `AmsTray` (unit/tray index, type, couleur hex, sous-marque, remain,
      tag_uid normalisé) ; un rapport sans `print.ams` → liste vide (ou
      « rapport partiel », pas d'erreur) ; le parsing du **statut** 22b reste
      identique (tests existants verts).
- [ ] `fetch_ams_trays` : Bambu retourne les Trays ; un Machine Link
      PrusaLink/Moonraker → indisponible sans planter ; timeout → erreur, aucune
      mutation.
- [ ] Domaine spools : `ams_tag_uid` persiste ; `memorize_ams_tag` stocke un UID
      réel, est idempotent, refuse/ignore un UID vide ou `0000…`.
- [ ] Confirmation : mémorise l'UID sur la bobine choisie **et** charge le Slot
      via `load_slot` ; l'exclusivité déplace une bobine déjà chargée ailleurs ;
      Empty/Archived reste non chargeable ; printer/slot inconnu → 404 ; le poids
      restant Filature **n'est pas** modifié.
- [ ] SPI (testcontainers) : la requête réconciliable exclut Empty/Archived et
      déjà-chargées, expose material_name/couleur/`ams_tag_uid` ; l'export/import
      d'instance round-trip `ams_tag_uid` ; un export d'avant 23 s'importe (UID
      absent).
- [ ] Web : le bouton « Synchroniser l'AMS » n'apparaît que sur une Printer Bambu
      avec Machine Link + ≥ 1 AMS Unit ; il ouvre le panneau (htmx) ; la
      confirmation charge les Slots et rafraîchit la carte ; feature désactivée
      sur l'instance démo.
- [ ] Parcours e2e : Printer Bambu + AMS Unit + Machine Link → synchro → le
      panneau suggère une bobine par Tray → correction d'une ligne → confirmation
      → les Slots AMS montrent les bobines chargées, l'exclusivité tient, le poids
      Filature est inchangé → seconde synchro → la bobine au `tag_uid` mémorisé
      est re-suggérée automatiquement (match certain).
- [ ] Glossaire (**AMS Tray**, **AMS Tag UID**, **Réconciliation AMS**) et
      ADR-0007 respectés. i18n EN + FR pour tous les nouveaux libellés. Capteurs
      domain-purity + hexagonal verts ; suite complète verte ; build offline +
      clippy propres ; cache `.sqlx/` à jour.

**Out of scope (YAGNI):**
- **Écrasement / alignement du poids** depuis `remain` AMS : la slice **affiche**
  l'écart en lecture seule ; l'action « aligner le poids » (option 3 de la
  discovery) attend une décision d'UI (modale ?) en phase design — follow-up
  éventuel `23b`.
- **Création automatique** d'une bobine pour un Tray sans match (jamais).
- **Prusa MMU / Klipper** : aucun Tray exposé — feature inerte.
- Écriture/relecture de RFID au-delà du `tag_uid` mémorisé ; historique de
  synchro ; routing bac→tête ; chargement multi-bobine par Slot.
- Toute modification du parsing **statut** 22a/22b ou des règles de chargement
  `15b` (on les réutilise, on ne les change pas).

**Design (phase 3b) — DÉCIDÉ (review lavish 2026-07-24) :**
- **Direction A — Panneau in-place.** Fragment htmx (`hx-get`) déclenché par
  « Synchroniser l'AMS » qui **remplace la zone Slots** de la carte. Une ligne
  par bac AMS, empilée 2 niveaux (haut : clé Slot `ams{u}-{n}` + bac détecté +
  écart poids lecture seule ; bas : `↳` + select bobine suggérée pleine largeur +
  badge match `RFID`/`attr.`/`aucun`). Footer confirmation groupée « Confirmer (n) ».
- **Multi-AMS** : lignes **groupées par AMS Unit** (sous-en-têtes `AMS 1/2…`) en
  grille 2 colonnes. Détail + justification (vs modale / par-slot) dans
  `docs/design.md` § « AMS reconciliation panel (slice 23) ». Maquette de travail :
  `.lavish/ams-reconciliation-design.html`.

**References:**
- Product brief : `docs/product/brief.md`
- Slices amont : `docs/specs/22a-machine-link-rest.md`,
  `docs/specs/22b-machine-link-bambu.md`, `docs/specs/17-printer-ams-topology.md`,
  `docs/specs/15b-printer-spool-loading.md`
- Transfert d'instance : `docs/specs/12-instance-export-import.md`
- ADRs : `docs/adr/0007-ams-reconciliation.md`, `0006-machine-credentials-encryption.md`,
  `0004-net-weight-no-tare.md`
- Glossaire : `docs/glossary.md` (§ « AMS spool sync », « Printers & filament loading »,
  « Machine connectivity »)
- Délégation Codex : voir mémoire `codex-delegation-workflow` (worktree,
  aws-lc-rs, advisories RustSec, gate Docker `tools/test.sh`)
