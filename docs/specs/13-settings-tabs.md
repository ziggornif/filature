## Agent Brief

**Category:** feature
**Summary:** Enrichir l'écran Paramètres en un shell à 4 onglets (Général · Fabricants · Emplacements · Sauvegarde) et retirer Fabricants + Emplacements de la nav principale.

**Slice / context:**
Slice UX de réorganisation de la navigation. Aujourd'hui la sidebar expose 6 items
de premier niveau (Tableau de bord, Bobines, Matériaux, **Fabricants**,
**Emplacements**, Paramètres). Fabricants et Emplacements sont des référentiels
peu volatils (saisis une fois, rarement modifiés) : la décision de design (handoff
UI, juillet 2026) est qu'ils ne sont **pas** des items de nav de premier niveau mais
des sous-onglets de l'écran Paramètres. Matériaux **reste** en nav principale
(consulté au quotidien).

Le back-end existe déjà et ne change pas : les référentiels Manufacturer et Location
(listing + création + suppression), la configuration d'instance (seuil de stock bas)
et le transfert d'instance (export/import JSON) sont tous implémentés et testés.
Cette slice est une **restructuration de la couche présentation uniquement** :
aucun changement de domaine, de port, de persistence, ni de logique métier.

Aujourd'hui l'écran Paramètres empile deux sections (« Stock » = seuil, « Transfert »
= export/import). Il doit devenir un shell à onglets ; les référentiels Fabricants et
Emplacements, aujourd'hui servis comme pages autonomes, migrent dans ce shell.

**Desired behavior:**

*Navigation (shell présent partout)*
- La sidebar n'expose plus que 4 items de premier niveau : Tableau de bord, Bobines,
  Matériaux, Paramètres. Les items Fabricants et Emplacements disparaissent de la nav.
- Le reste du shell (logo, compteur de bobines sur l'item Bobines, menu burger mobile,
  tagline, item actif) est inchangé.

*Shell Paramètres à onglets*
- L'écran Paramètres affiche une barre de 4 onglets dans cet ordre : **Général**,
  **Fabricants**, **Emplacements**, **Sauvegarde**. L'onglet actif porte un
  soulignement 2px en couleur d'accent (`--accent`) ; les inactifs sont en `--muted`.
- Chaque onglet est un **lien `<a href>` réel** vers sa sous-route, la barre d'onglets
  portant `hx-boost="true"` : htmx intercepte le clic, fait un GET AJAX, swappe le
  `<body>` et pousse l'URL automatiquement (deep-link + bouton précédent gratuits). Si
  JS est désactivé, le lien retombe sur une navigation pleine page classique — les
  routes rendent de toute façon la page complète. Pas de négociation fragment/page ni
  d'endpoint supplémentaire pour les onglets.
- Les 4 onglets correspondent à 4 routes GET :
  - `GET /settings` → onglet **Général** actif : réglage du seuil de stock bas
    (même contrôle et même contrat htmx qu'aujourd'hui : POST `/settings/low-stock-threshold`
    renvoyant le fragment re-rendu).
  - `GET /settings/manufacturers` → onglet **Fabricants** actif : table des fabricants
    (colonnes actuelles conservées, dont **Pays** et le nombre de bobines référençantes)
    + formulaire d'ajout. Les contrats POST `/manufacturers` et DELETE de fabricant
    restent inchangés (fragments de ligne).
  - `GET /settings/locations` → onglet **Emplacements** actif : table des emplacements
    + formulaire d'ajout, contrats POST `/locations` et DELETE inchangés.
  - `GET /settings/backup` → onglet **Sauvegarde** actif : export (téléchargement JSON
    complet) + import (file input, limite 1 Mio, case de confirmation obligatoire,
    bouton rouge de remplacement). Contrats GET `/settings/export` et POST
    `/settings/import` inchangés, y compris la redirection `?imported=1` et les
    messages d'erreur d'import.

*Suppression des anciennes pages*
- Les pages autonomes GET `/manufacturers` et GET `/locations` n'existent plus :
  ces URLs renvoient **404**. (Aucune redirection.)
- Les endpoints POST/DELETE des référentiels (fragments) restent servis aux mêmes
  chemins qu'aujourd'hui ; seules les pages GST de listing autonomes disparaissent.

*Comportement conservé*
- L'intégrité référentielle actuelle est conservée telle quelle : un Fabricant ou un
  Emplacement encore référencé par une bobine ne peut être supprimé (comportement et
  message existants, cf. glossaire).
- Thème clair/sombre, i18n (fr/en), et le rendu server-side + htmx restent la cible ;
  aucune dépendance JS front ni build front n'est introduite.

**Key interfaces:** (vocabulaire glossaire + contrats de route — sans chemins de fichier)
- Écran **Paramètres** : shell rendu côté serveur exposant un `active_tab` (une des
  valeurs `general` | `manufacturers` | `locations` | `backup`) qui pilote l'onglet
  souligné et le panneau affiché.
- Route `GET /settings` (Général), `GET /settings/manufacturers`,
  `GET /settings/locations`, `GET /settings/backup` — chacune charge sa donnée
  (configuration d'instance / liste Manufacturer / liste Location) et rend le shell
  avec l'onglet correspondant actif.
- **Manufacturer** / **Location** (glossaire) : referentials listés/créés/supprimés ;
  leurs cas d'usage et fragments de ligne sont réutilisés tels quels dans les onglets.
- Contrats inchangés à préserver : POST `/settings/low-stock-threshold`,
  GET `/settings/export`, POST `/settings/import`, POST `/manufacturers`,
  DELETE fabricant, POST `/locations`, DELETE emplacement.
- Nouvelles clés i18n (fr/en) pour les libellés d'onglets
  (`settings.tab.general` / `.manufacturers` / `.locations` / `.backup`) ; réutiliser
  les clés existantes des référentiels et du transfert.

**Acceptance criteria:**
- [ ] La sidebar ne contient plus de lien vers `/manufacturers` ni `/locations` ;
      elle contient Tableau de bord, Bobines, Matériaux, Paramètres.
- [ ] `GET /settings` rend une barre de 4 onglets (Général, Fabricants, Emplacements,
      Sauvegarde) avec Général actif (soulignement accent) et le réglage de seuil de stock.
- [ ] `GET /settings/manufacturers` rend le shell avec l'onglet Fabricants actif, la
      table des fabricants (colonne Pays incluse) et le formulaire d'ajout fonctionnel.
- [ ] `GET /settings/locations` rend le shell avec l'onglet Emplacements actif, la table
      des emplacements et le formulaire d'ajout fonctionnel.
- [ ] `GET /settings/backup` rend le shell avec l'onglet Sauvegarde actif, l'export et
      l'import (limite 1 Mio, case de confirmation obligatoire).
- [ ] Ajout et suppression d'un fabricant / emplacement fonctionnent depuis leur onglet
      (fragments htmx swappés en place), sans rechargement complet.
- [ ] La suppression d'un fabricant/emplacement encore référencé est refusée avec le
      message existant (intégrité conservée).
- [ ] `GET /manufacturers` et `GET /locations` renvoient 404.
- [ ] La barre d'onglets porte `hx-boost` sur des `<a href>` réels : le changement
      d'onglet swappe sans reload visuel complet, pousse l'URL (bouton précédent OK), et
      chaque sous-route reste accessible en accès direct / sans JS (fallback page complète).
- [ ] POST `/settings/low-stock-threshold`, GET `/settings/export`, POST `/settings/import`
      conservent leur comportement (persistance, redirection `?imported=1`, erreurs).
- [ ] L'onglet actif et les panneaux s'affichent correctement en thème clair et sombre,
      en fr et en en, et restent lisibles sur mobile (rail d'icônes de la sidebar inchangé).
- [ ] Toute la suite de tests passe (unit + intégration + e2e), tests de nav/pages mis à
      jour pour refléter le 404 des anciennes URLs et la présence des onglets.

**Out of scope:**
- Aucun changement de domaine, port (API/SPI), persistence, ou logique métier.
- Pas d'écran Humidité / dryboxes (déféré post-v1).
- Pas de modification des colonnes ou de la sémantique des référentiels (la colonne Pays
  des fabricants est conservée, pas retirée ni ajoutée d'autres).
- Pas de nouvelle règle d'intégrité référentielle (on garde le comportement actuel).
- Pas de redirection depuis les anciennes URLs (elles renvoient 404, décision actée).
- Barre d'onglets en `hx-boost` sur des `<a href>` réels (décision actée) : pas de
  pattern tabs-hateoas (fragments + négociation `HX-Request`), pas de `hx-push-url`
  manuel, pas d'endpoint fragment dédié aux onglets.
- Pas d'introduction de framework/JS front ni de build front.

**References:**
- Product brief : `docs/product/brief.md`
- Handoff UI (source du design onglets + nav) : `init_assets/design_handoff_filature/`
  et README du UI Kit (§ Navigation, § 6bis. Paramètres)
- Design : `docs/design.md`
- Glossary : `docs/glossary.md` (Manufacturer, Location, Material)
- Slices amont réutilisées : `docs/specs/04-locations.md`,
  `docs/specs/10-settings-alert-threshold.md`, `docs/specs/12-instance-export-import.md`

---
*Brief rédigé par Claude Code (orchestrateur). Implémentation déléguée à Codex.*
