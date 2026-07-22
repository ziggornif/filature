# 22b — Connexion machine (Bambu LAN) : statut live via MQTT

> Brief généré par IA (harness) à partir du design validé en review lavish le
> 2026-07-22, relu par un humain. **Conditionné au go du spike Bambu.**

## Préalable — spike jetable (go/no-go)

Avant toute implémentation : un spike **jetable** (code hors repo ou supprimé
ensuite) contre la Bambu réelle du parc, qui valide :
1. le mode LAN (dev mode) est activable et l'accès MQTT fonctionne sur le
   firmware actuel de la machine ;
2. connexion MQTT TLS :8883 avec cert auto-signé + access code LAN, abonnement
   `device/{serial}/report`, réception d'un payload d'état exploitable
   (état, progression, job, températures) ;
3. la stratégie « connexion courte » (connect → premier report → disconnect)
   répond en < 3 s ; sinon, mesurer et documenter l'alternative (connexion
   maintenue côté serveur).
Sortie du spike : go/no-go + payload réel documenté (collé dans l'issue du
spike ou en commentaire de celle de 22b). **No-go → cette slice est gelée**,
pas de demi-implémentation.

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
   code, abonnement `device/{serial}/report`, premier message d'état → mappé
   en `MachineStatus`, disconnect. Timeout global court (~3 s) ; échec de
   connexion, d'auth ou timeout → Offline. Si le spike a conclu qu'une
   connexion maintenue est nécessaire, l'adapter la gère en interne — le port
   `MachineStatusProbe` et le reste de l'app ne changent pas.
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
- [ ] Acceptation du cert auto-signé confinée à l'adapter Bambu — les clients HTTP de 22a gardent la validation TLS par défaut.
- [ ] Access code chiffré en DB, jamais ré-affiché, absent de tout HTML/fragment.
- [ ] « Tester la connexion » opérationnel pour la variante Bambu.
- [ ] Payload `report` avec état inconnu → état sûr, couvert par un test.
- [ ] Tests adapter contre broker MQTT factice (nominal, auth KO, timeout, payload inattendu) ; validation manuelle contre la machine réelle documentée dans la PR. Suite complète verte.

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
