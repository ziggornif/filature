# 22b — Connexion machine (Bambu LAN) : statut live via MQTT

> Brief généré par IA (harness) à partir du design validé en review lavish le
> 2026-07-22, relu par un humain. **Go du spike Bambu validé dans l'issue #92.**

## Résultats du spike Bambu réel (issue #92) — acquis

- [x] Le mode LAN fonctionne et expose MQTT TLS sur le port `8883`, avec
  l'utilisateur `bblp` et l'access code comme mot de passe.
- [x] Le certificat est auto-signé. Son acceptation doit rester confinée au
  seul adapter Bambu. **TLS 1.2 doit être sélectionné explicitement** : la
  négociation par défaut échoue avec `Protocol error`.
- [x] Les séries P1 émettent des pushs incrémentaux : le premier message est
  partiel. La séquence validée est connect → abonnement à
  `device/{serial}/report` → publication de
  `{"pushing":{"sequence_id":"1","command":"pushall"}}` sur
  `device/{serial}/request` → attente du rapport contenant
  `print.gcode_state` → disconnect.
- [x] Le cycle complet mesuré est d'environ 2 s ; le timeout global reste à
  environ 3 s, au-delà duquel la machine est considérée Offline.
- [x] `print.mc_remaining_time` est exprimé en **minutes** et doit être
  converti en secondes pour `MachineTelemetry.remaining_seconds`.

## Agent Brief

**Category:** feature
**Summary:** Étendre la Machine Link aux Printers Bambu Lab (IP + access code LAN + numéro de série, transport MQTT) pour qu'elles affichent leur Machine Status comme les machines REST de 22a.

**Slice / context:**
S'appuie sur `22a-machine-link-rest`, qui a posé Machine Link, Machine Status,
le port SPI `MachineStatusProbe`, le chiffrement (ADR-0006), les fragments de
statut (cartes + Farm Activity) et le bouton de test. Cette slice ajoute
uniquement la variante Bambu : une troisième forme de Machine Link et un
adapter MQTT. L'UI de statut existe déjà et ne change pas.

**Desired behavior:**

1. **Configuration** — pour une Printer Bambu Lab, la section « Connexion
   machine » propose : hôte/IP + access code LAN + numéro de série. L'access
   code est chiffré au repos et jamais ré-affiché (mêmes règles que la clé
   PrusaLink, ADR-0006) ; hôte et numéro de série restent en clair.
2. **Adapter MQTT** — l'adapter Bambu implémente `MachineStatusProbe` en
   « connexion courte » : connect TLS :8883 (acceptation du cert auto-signé
   **limitée à cet adapter**, jamais globale), authentification par access
   code, abonnement `device/{serial}/report`, publication de `pushall`, puis
   attente du premier rapport **complet** contenant `print.gcode_state` →
   mapping en `MachineStatus`, disconnect. Timeout global court (~3 s) ; échec
   de connexion, d'auth ou timeout → Offline. Le port `MachineStatusProbe` et
   le reste de l'app ne changent pas.
3. **Statut affiché** — badge Machine State, bloc job (progression, temps
   restant, nom, températures) sur la carte et dans Farm Activity, à
   l'identique des machines REST — aucune UI nouvelle.
4. **Test de connexion** — le bouton « tester la connexion » fonctionne pour
   la variante Bambu (succès = état détecté ; échec = erreur claire).
5. **Mapping des états** — les états Bambu (payload `report`) sont mappés vers
   les Machine States du glossaire ; tout état inconnu/inattendu est mappé sur
   un état sûr (Idle ou Error selon le contexte du payload), jamais un crash.

**Key interfaces:** (glossaire : Printer, Machine Link, Machine Status, Machine State)
- `MachineLink` — nouvelle variante Bambu LAN (host + access code + serial),
  mêmes règles de persistance chiffrée que 22a.
- `MachineStatusProbe` — troisième adapter (MQTT) ; le port ne change pas.
- Client MQTT en couche infrastructure uniquement (ex. `rumqttc`, précédent du
  spike humidité) — le domaine reste pur, aucun MQTT/TLS dans `domain`.

**Acceptance criteria:**
- [ ] Bambu réelle en LAN mode avec IP + access code + série valides : badge et bloc job conformes à la machine, sur la carte et dans Farm Activity.
- [ ] Machine éteinte / access code invalide / série inconnue : badge Offline sous ~3 s, aucun blocage de page, aucun panic.
- [x] Acceptation du cert auto-signé confinée à l'adapter Bambu — les clients HTTP de 22a gardent la validation TLS par défaut.
- [x] Access code chiffré en DB, jamais ré-affiché, absent de tout HTML/fragment.
- [x] « Tester la connexion » implémenté pour la variante Bambu.
- [x] Payload `report` avec état inconnu → état sûr, couvert par un test.
- [x] Parsing/mapping MQTT couvert en unitaire (rapports complets RUNNING/IDLE,
  push partiel, état inconnu). Le broker factice est écarté comme
  disproportionné ; auth KO et timeout suivent le chemin d'erreur de
  l'adapter. La validation manuelle finale contre la machine réelle reste à
  documenter dans la PR.

**Out of scope:**
- Synchro des bobines chargées / AMS (RFID) — slice `23`.
- Bambu cloud (mode non-LAN) — LAN uniquement.
- Toute évolution de l'UI de statut, du panneau Farm Activity ou du modèle `MachineStatus` posés par 22a.
- Contrôle machine, caméra, historique, notifications.

**References:**
- Spec amont : `docs/specs/22a-machine-link-rest.md`
- ADR chiffrement : `docs/adr/0006-machine-credentials-encryption.md`
- Glossaire : `docs/glossary.md` (§ Machine connectivity)
- Brief produit : `docs/product/brief.md` (job #5)
