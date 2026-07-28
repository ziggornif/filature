# 24 — Stub Machine Link pour instance de démo

> Brief généré par IA (harness) pour outiller les captures de promo. Outillage
> de démo : rien de ce qui est ajouté ici ne doit pouvoir s'activer sur une
> instance de production.

## Agent Brief

**Category:** tooling
**Summary:** Permettre de faire tourner une instance de démo locale où les
Machine Status des imprimantes sont **simulés depuis un fichier JSON** (aucune
machine, aucun réseau), afin de produire des captures d'écran du dashboard et
de la vue Imprimantes avec des machines dans des états variés (dont plusieurs
en cours d'impression).

**Slice / context:**
Les slices `22a` (REST Prusa/Moonraker), `22b` (Bambu MQTT) et `23`/`23b`
(synchro AMS) affichent des statuts live via le port SPI `MachineStatusProbe`
(`crates/domain/src/printers/ports/spi.rs`), implémenté par
`crates/app/src/machine_http.rs::MachineStatusProbeAdapter` et câblé dans
`crates/app/src/web/state.rs:65`. Sans machine réelle sur le LAN, toutes les
cartes affichent `Offline` et le panneau Farm Activity est vide : impossible de
faire une capture représentative. Le mode `FILATURE_DEMO=1` existant fait
l'inverse de ce qu'on veut ici (il **masque** Machine Link et Farm Activity
pour l'instance de démo publique) — il n'est pas modifié par cette slice.

**Desired behavior:**

1. **Feature cargo `demo-stub`** sur le crate `filature` (`crates/app`), **non
   activée par défaut**. Tout le code ajouté par cette slice est derrière
   `#[cfg(feature = "demo-stub")]`. Sans la feature, le binaire est
   bit-à-bit fonctionnellement identique à aujourd'hui (aucun chemin
   d'exécution, aucune dépendance en plus).
2. **Adapter stub** — nouveau module `crates/app/src/machine_stub.rs` exposant
   `DemoMachineStatusProbe`, implémentant `domain::printers::MachineStatusProbe` :
   - `DemoMachineStatusProbe::from_file(path: &str) -> Result<Self, String>` :
     charge et valide le scénario JSON une fois au boot.
   - `fetch_status(&MachineLink)` : renvoie le `MachineStatus` du scénario
     correspondant à la clé de la link ; clé inconnue → `MachineStatus::offline()`.
   - `fetch_ams(&MachineLink)` : pour une `MachineLink::BambuLan`, renvoie les
     `AmsTray` du scénario (liste vide si le scénario n'en déclare pas) ; pour
     toute autre variante, `Err(MachineError::AmsUnavailable)` — même contrat
     que `MachineStatusProbeAdapter`.
   - Aucune I/O réseau, aucun sleep, aucun état mutable : réponses
     déterministes (une même capture est reproductible).
3. **Clé de scénario** (`key_for(&MachineLink)`), à documenter en commentaire
   dans le module :
   - `PrusaLink { host, .. }` → `host`
   - `Moonraker { url }` → `url`
   - `BambuLan { serial, .. }` → `serial` (identifiant stable, indépendant de l'IP)
4. **Activation** — env `FILATURE_MACHINE_STUB=<chemin du fichier JSON>`, lue
   dans `web/state.rs` :
   - feature `demo-stub` absente → la variable est **ignorée** (adapter réel) ;
   - feature présente + variable absente → adapter réel, comportement inchangé ;
   - feature présente + variable définie → adapter stub, précédé d'un
     `tracing::warn!` explicite (« statuts machine SIMULÉS depuis <chemin> —
     instance de démo ») ;
   - feature présente + variable définie + fichier illisible ou JSON invalide →
     **échec bruyant au démarrage** (`expect` avec message clair, cohérent avec
     l'`expect` déjà présent ligne 67), jamais de repli silencieux.
5. **Sous-commande `filature encrypt-credential <valeur>`**, elle aussi derrière
   `#[cfg(feature = "demo-stub")]`, sur le même modèle que `hash-password` dans
   `main.rs` (traitée avant tout setup serveur/DB) : chiffre la valeur avec
   `CredentialCipher::from_env()` et imprime le texte chiffré base64 attendu par
   la colonne `machine_links.credential`. Sans `FILATURE_CREDENTIALS_KEY`, erreur
   explicite + `exit(2)`. Elle sert uniquement au seed de démo (les liens
   machine ne transitent pas par l'export/import d'instance).
6. **Fichier de scénario** `tools/demo-machines.json` — contenu exact fourni
   plus bas (§ Scénario de démo).
7. **Script de seed** `tools/seed-demo.sh` (bash, `set -euo pipefail`,
   exécutable), documenté en tête, qui prend une instance déjà démarrée et :
   - **refuse de s'exécuter sans `--yes` explicite, et refuse toute
     `FILATURE_URL` dont l'hôte n'est pas `localhost`/`127.0.0.1`/`::1`** :
     l'import étant un remplacement complet, une simple inversion d'URL
     détruirait une instance réelle. L'en-tête du script porte l'avertissement ;
   - se logue (`POST /login`, cookie jar) puis importe `tools/demo-instance.json`
     via `POST /settings/import` (`confirm_replace=yes`, `backup=@…`) — l'import
     **remplace** tout et supprime en cascade les `machine_links` existantes,
     donc il passe **avant** l'insertion des liens ;
   - insère ensuite les lignes `machine_links` en SQL (`psql`) pour les
     imprimantes listées § Liens machine, en chiffrant les credentials via
     `filature encrypt-credential` ;
   - échoue explicitement si un `printer_id` attendu n'existe pas en base
     (dataset et script désynchronisés) ;
   - est paramétrable par env avec des défauts locaux :
     `FILATURE_URL` (défaut `http://127.0.0.1:8080`), `FILATURE_USER`/`FILATURE_PASSWORD`
     (défaut `demo`/`demo`), `DATABASE_URL`
     (défaut `postgres://filature:filature@127.0.0.1:5432/filature`).
8. **Documentation** — une section « Instance de démo » dans `CONTRIBUTING.md`
   (ou `README.md` si plus logique) donnant la recette complète : Postgres,
   `FILATURE_CREDENTIALS_KEY`, `cargo run -p filature --features demo-stub`,
   `FILATURE_MACHINE_STUB=tools/demo-machines.json`, `tools/seed-demo.sh`.
   Mentionner que `FILATURE_DEMO` doit rester **non défini** (sinon Machine Link
   et Farm Activity sont masqués).

**Key interfaces:**
- `domain::printers::MachineStatusProbe` (SPI) — seul point d'extension utilisé.
  **Le domaine n'est pas modifié par cette slice** : aucun fichier de
  `crates/domain/` ne doit changer.
- `MachineStatus` / `MachineTelemetry` / `Temperature` / `AmsTray` — types
  domaine construits par le stub (`crates/domain/src/printers/machine.rs`).
- `crate::credentials::CredentialCipher` — réutilisé tel quel pour la
  sous-commande de chiffrement.

**Format du scénario JSON:**

```json
{
  "machines": {
    "<clé = host | url | serial>": {
      "state": "printing | idle | paused | error | offline",
      "progress_percent": 68,
      "remaining_seconds": 4380,
      "job_name": "support-cam-v3.3mf",
      "active_head": 0,
      "nozzles": [{ "actual": 218.0, "target": 220.0 }],
      "bed": { "actual": 60.0, "target": 60.0 },
      "ams": [
        {
          "unit_index": 0,
          "tray_index": 0,
          "filament_type": "PLA",
          "color_hex": "1A1A1AFF",
          "sub_brand": "PLA Basic",
          "remain_percent": 78,
          "tag_uid": "A1B2C3D4E5F60001"
        }
      ]
    }
  }
}
```

Tous les champs hors `state` sont optionnels (`serde(default)`), `state` est
obligatoire ; une valeur d'état inconnue est une erreur de chargement. Une clé
en trop dans le JSON doit être refusée (`deny_unknown_fields`) pour qu'une
faute de frappe se voie au boot plutôt que dans une capture.

**Scénario de démo (`tools/demo-machines.json`) :**

| Clé (host/url/serial) | Imprimante | État | Détail |
|---|---|---|---|
| `01P00A3B0500123` | P1S Atelier (bambu) | printing | 68 %, 4380 s, `support-cam-v3.3mf`, buse 218/220, plateau 60/60, AMS 3 plateaux cohérents avec les bobines chargées |
| `01H00C7D0900456` | H2D Prod (bambu) | printing | 24 %, 9120 s, `boitier-capteur-v2.3mf`, 2 buses (`active_head: 1`, 255/255 et 28/0), plateau 90/90, AMS avec **1 plateau divergent** (montre le drift de la slice 23) |
| `01X00E1F0300789` | X2D Bench (bambu) | idle | buse 26/0, plateau 24/0 |
| `01H00C2A0700321` | H2C Studio (bambu) | paused | 41 %, 5400 s, `capot-ventilo.3mf`, buse 245/245, plateau 100/100 |
| `192.168.1.42` | XL Ferme (prusalink) | printing | 91 %, 720 s, `plaque-montage.bgcode`, `active_head: 0`, buse 250/250, plateau 90/90 |
| `192.168.1.43` | Core One Bureau (prusalink) | error | pas de job, buse 31/0, plateau 25/0 |
| `192.168.1.44` | MK4S Etabli (prusalink) | idle | buse 27/0, plateau 24/0 |
| `http://192.168.1.51:7125` | Qidi X-Max 3 (moonraker) | printing | 12 %, 11700 s, `pa612-support-bras.gcode`, buse 270/270, plateau 90/90 |

Ender 3 n'a **pas** de Machine Link (cas « imprimante non connectée » visible
dans la même capture). Les noms de job, couleurs AMS et matières doivent rester
cohérents avec `tools/demo-instance.json`.

**Liens machine à insérer par `tools/seed-demo.sh` :**

| `printer_id` (issu de `tools/demo-instance.json`) | `kind` | `endpoint` | `credential` |
|---|---|---|---|
| `01KXPB0RJFEK6A2FZTP900VT9T` (P1S Atelier) | `bambu` | `{"host":"192.168.1.31","serial":"01P00A3B0500123"}` | chiffré (`12345678`) |
| `01KXPB0RM6NTG2JM083TC7W7FK` (H2D Prod) | `bambu` | `{"host":"192.168.1.32","serial":"01H00C7D0900456"}` | chiffré (`12345678`) |
| `01KXPB0RP0AW0FN5H6KZ3HV047` (X2D Bench) | `bambu` | `{"host":"192.168.1.33","serial":"01X00E1F0300789"}` | chiffré (`12345678`) |
| `01KXPB0RQF4JSM2J0DXWJ735YF` (H2C Studio) | `bambu` | `{"host":"192.168.1.34","serial":"01H00C2A0700321"}` | chiffré (`12345678`) |
| `01KXPB0RRSEWBJ6MMVDZDVYN7W` (XL Ferme) | `prusalink` | `192.168.1.42` | chiffré (`demo-api-key`) |
| `01KXPB0RT0ZFWRWHR3H89E586A` (Core One Bureau) | `prusalink` | `192.168.1.43` | chiffré (`demo-api-key`) |
| `01KXPB0RV93GKGAHCM73K67XM3` (MK4S Etabli) | `prusalink` | `192.168.1.44` | chiffré (`demo-api-key`) |
| `01KXPB0RWFG622WZS7KTZK1VYV` (Qidi X-Max 3) | `moonraker` | `http://192.168.1.51:7125` | `NULL` |

Ces credentials sont des valeurs factices d'un jeu de démo (aucune machine
réelle) ; elles restent chiffrées en base par le chemin normal.

**Acceptance criteria:**
- [ ] `cargo build -p filature` (sans feature) : aucun changement de comportement, `FILATURE_MACHINE_STUB` ignorée, sous-commande `encrypt-credential` absente.
- [ ] `cargo run -p filature --features demo-stub` **sans** `FILATURE_MACHINE_STUB` : adapter réel, comportement identique à aujourd'hui.
- [ ] Avec la feature + `FILATURE_MACHINE_STUB=tools/demo-machines.json` : le dashboard affiche le panneau Farm Activity avec 4 machines en impression (progression, temps restant), et les cartes imprimantes affichent les badges `printing` / `paused` / `idle` / `error` conformes au tableau ci-dessus.
- [ ] Fichier de scénario absent ou JSON invalide (état inconnu, champ inconnu) → échec au démarrage avec un message nommant le fichier et la cause ; jamais de repli silencieux sur l'adapter réel.
- [ ] Clé absente du scénario → `Offline` (pas d'erreur, pas de panique).
- [ ] `fetch_ams` sur une link non-Bambu → `MachineError::AmsUnavailable` ; sur la link Bambu de H2D → plateaux dont un divergent, le drawer de réconciliation (23) affiche l'écart.
- [ ] `tools/seed-demo.sh` sur une instance vierge : import OK puis 8 lignes `machine_links` ; script relançable (idempotent) ; échec explicite si un `printer_id` manque.
- [ ] Aucun fichier de `crates/domain/` modifié.
- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` **avec et sans** `--features demo-stub` : verts.
- [ ] Tests unitaires du stub (chargement d'un scénario valide, mapping des 5 états, `key_for` pour les 3 variantes de link, clé inconnue → Offline, JSON invalide → Err) ; `tools/test.sh` (= `cargo test --workspace --all-features`) vert.

**Out of scope:**
- Toute modification du crate `domain`, des templates, du CSS ou de l'i18n.
- Toute modification du comportement `FILATURE_DEMO` existant.
- Progression animée dans le temps / statuts évolutifs : le stub est
  déterministe et statique.
- Faux serveurs HTTP ou broker MQTT (option écartée).
- Livraison du stub dans l'image Docker de production (`Dockerfile` inchangé :
  la feature reste hors build par défaut).
- Enrichissement de `tools/demo-instance.json` : **traité en parallèle hors de
  cette tâche**, ne pas y toucher.

**References:**
- Slices amont : `docs/specs/22a-machine-link-rest.md`, `22b-machine-link-bambu.md`, `23-ams-spool-sync.md`, `23b-ams-reconciliation-refinements.md`
- ADR chiffrement : `docs/adr/0006-machine-credentials-encryption.md`
- Port SPI : `crates/domain/src/printers/ports/spi.rs`
- Adapter réel : `crates/app/src/machine_http.rs`, `crates/app/src/machine_bambu.rs`
- Câblage : `crates/app/src/web/state.rs:55-76`
- Import d'instance : `crates/app/src/web/settings.rs` (route `/settings/import`)
