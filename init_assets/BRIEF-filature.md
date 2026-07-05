# Brief — Filature

Gestionnaire de stock de filament pour impression 3D, auto-hébergé, mono-binaire.

## 1. Contexte & objectif

Suivre un parc de bobines de filament réparti sur plusieurs imprimantes et dryboxes : savoir ce qu'il reste (poids, longueur, valeur), quand une bobine a été ouverte, et surtout **surveiller l'humidité des matériaux sensibles** en branchant les capteurs SHT31 déjà présents sur le homelab (relais MQTT). C'est ce dernier point qui justifie de faire son propre outil plutôt que d'utiliser Spoolman.

Cible : usage perso + Zig Factory (chiffrage coût matière d'une pièce). Déploiement sur k3s via un chart Helm, un seul binaire + un fichier SQLite.

## 2. Stack (décidée)

| Couche | Choix | Raison |
|---|---|---|
| Serveur HTTP | **Axum** | Écosystème Tokio/tower, `State` extractor propre, partage trivial de l'état avec la tâche MQTT en arrière-plan. Actix fait le job mais Axum compose mieux ici. |
| SQL | **SQLx** (SQLite) | Requêtes vérifiées à la compilation (`query!`), migrations embarquées. |
| Templating | **Tera** | Moteur Jinja-like file-based au runtime. Choisi pour la fluidité des partials htmx (`extends`/`include`/`macro`), le hot reload pendant l'intégration, et parce que le HTML produit par le design se colle quasi tel quel dans les `.html`. Contrepartie assumée : pas de vérif des templates à la compilation → compensée par des tests de rendu (§6). |
| Réactivité front | **htmx** en vanilla | Aucune toolchain npm. htmx servi en statique. |
| MQTT | **rumqttc** | Client async, tâche tokio dédiée qui alimente les relevés d'humidité. |
| Runtime | Tokio | — |

**Contraintes fermes :**
- Binaire unique auto-suffisant : migrations et assets statiques (htmx, CSS, templates) embarqués dans le binaire.
- Pas de JS custom au-delà de htmx.
- SQLite en mode WAL. Backup = copie de fichier (litestream envisageable plus tard vers le S3 Scaleway).
- Config par fichier TOML + surcharge par variables d'env.

## 3. Architecture

Hexagonal léger + slices verticales, **une seule crate** pour le MVP (le domaine pourra être extrait plus tard si besoin). Pragmatisme : des traits (ports SPI) là où ça achète de la testabilité, pas partout.

- **Ports driving (api)** = handlers Axum, par slice.
- **Ports driven (spi)** = trait repository par slice, implémenté sur SQLx. Les handlers dépendent du trait → domaine testable sans DB.
- Slices : `spools`, `materials`, `humidity`, `dashboard`.

## 4. Domaine (concepts)

Les entités à modéliser, sans figer le schéma — à Claude Code de proposer la structure exacte :

- **Spool** — la bobine physique : matériau, couleur, marque, diamètre, poids restant (pesée ou décrément), tare, rangement, statut (scellée/ouverte/vide/archivée), dates, fournisseur, prix. Reste dérivable en grammes **et** en mètres.
- **Material** — référentiel des matériaux (PLA, PETG, ASA, PA-CF, PC Blend, TPU…) avec densité, paramètres de séchage et sensibilité à l'humidité. Seedé au démarrage, éditable.
- **Location** — un rangement (drybox, étagère), éventuellement associé à un topic MQTT.
- **HumidityReading** — relevé horodaté (%HR, température) rattaché à un rangement, alimenté par la tâche MQTT.
- **ConsumptionEvent** — consommation de filament par job (poids/coût). Périmètre ultérieur.

## 5. Périmètre fonctionnel

**Phase 1 — cœur stock (MVP)**
CRUD matériaux (avec seed) et bobines. Mise à jour rapide du poids restant (pesée directe ou « j'ai consommé X g »). Liste filtrable/triable sans rechargement. Détail bobine. Dashboard : valeur du stock, poids restant, répartition par matériau, bobines bientôt vides.

**Phase 2 — humidité (le différenciateur)**
CRUD rangements avec topic MQTT. Tâche d'abonnement MQTT qui insère les relevés. Panneau humidité par drybox, **statut coloré selon la sensibilité du matériau rangé** (PA-CF à 45 % HR → « à sécher »). Rafraîchissement live.

**Plus tard**
Consommation par job, coût matière par impression, historique par machine, export PDF/CSV de l'inventaire.

## 6. Qualité

- Tests unitaires sur le domaine : calcul longueur↔poids, décrément de stock, statut humidité vs seuil matériau.
- Un test d'intégration par slice avec une SQLite en mémoire.
- **Test de rendu de chaque template** (page et fragment) : Tera ne vérifiant rien à la compilation, un test qui rend chaque template avec un contexte représentatif attrape les fautes de variable au `cargo test` plutôt qu'en prod. Ne pas le sauter.
- `cargo clippy` propre, `rustfmt`.
- Pas de sur-abstraction : traits sur les repos oui, hiérarchie d'interfaces gratuite non.

## 7. Design & UI

**Toute la partie design est dans `design_handoff_filature/`** — maquettes des écrans, système de composants, thèmes clair/sombre (défaut OS), palette et interactions htmx. S'y référer pour l'implémentation front ; ne pas réinventer l'UI dans ce brief.

## 8. Déroulé attendu

Procéder **slice par slice, incrémentalement**, avec un point de commit compilable et testé à chaque étape. Commencer par valider le modèle de domaine (types + migration initiale) avant d'écrire les handlers, puis dérouler : materials → spools → dashboard → locations/humidity. Le périmètre ultérieur (consommation, coût, export) seulement après validation des phases 1 et 2.
