# 22a — Connexion machine (REST) : statut live Prusa + Moonraker

> Brief généré par IA (harness) à partir du design validé en review lavish le
> 2026-07-22, relu par un humain.

## Agent Brief

**Category:** feature
**Summary:** Permettre d'attacher une Machine Link (PrusaLink ou Moonraker) à une Printer et afficher son Machine Status en direct — badge d'état sur les cartes imprimantes et panneau Farm Activity sur le dashboard.

**Slice / context:**
S'appuie sur les slices Printers (`15a`…`17`, `21`) et le dashboard (`05`).
Aujourd'hui la vue Imprimantes n'affiche que le déclaratif (topologie, bobines
chargées) ; connaître l'état réel d'une machine impose d'ouvrir PrusaLink ou
Fluidd/Mainsail. Aucune connectivité réseau machine n'existe. Les termes
Machine Link / Machine Status / Machine State / Farm Activity sont actés au
glossaire ; le chiffrement des credentials est acté par l'ADR-0006.

**Desired behavior:**

1. **Configuration** — le formulaire imprimante (création + édition) gagne une
   section « Connexion machine » optionnelle, adaptée à la Printer Brand :
   - **Prusa** : hôte/IP + clé API PrusaLink.
   - **Other** : toggle « machine Klipper » qui révèle un champ URL de l'API
     Moonraker (URL seule — pas de clé API dans cette slice).
   - **Bambu Lab** : hors de cette slice (22b) — pas de section connexion.
   Une Printer sans Machine Link se comporte exactement comme aujourd'hui.
2. **Bouton « tester la connexion »** dans la section : déclenche
   `test_machine_link` avec les valeurs saisies et affiche le résultat
   (machine détectée + Machine State, ou erreur) sans soumettre le formulaire.
3. **Secrets** — la clé API PrusaLink est chiffrée au repos (AES-256-GCM, env
   `FILATURE_CREDENTIALS_KEY`, cf. ADR-0006). En édition, elle n'est jamais
   ré-affichée : placeholder « configuré ». Hôte et URL restent en clair.
   Aucun credential ne transite vers le navigateur, ni dans les fragments.
4. **Statut sur les cartes imprimantes** — chaque carte d'une Printer avec
   Machine Link charge un fragment htmx de statut (`hx-get` au chargement,
   re-poll toutes les ~10 s) affichant :
   - badge Machine State dans le header, **à côté** du badge d'occupation de
     la slice 21 (coexistence actée) : Offline (neutre, bord pointillé),
     Idle (neutre), Printing (`--ok`), Paused (`--warn`), Error (`--danger`) ;
   - si Printing/Paused : bloc job — nom du fichier (tronqué), barre de
     progression, % + temps restant, températures buse/plateau (tête active ;
     repli première tête si l'API ne dit pas laquelle est active).
   La requête machine est proxifiée côté serveur avec un timeout court
   (2-3 s) ; machine injoignable → badge Offline, jamais d'erreur bloquante ;
   la page ne bloque jamais sur une machine éteinte (fragment asynchrone).
   Aucun Machine Status n'est persisté.
5. **Farm Activity (dashboard)** — nouveau panneau « Activité du parc » :
   une ligne par Printer **ayant une Machine Link** (les autres n'y
   apparaissent pas) avec nom, badge Machine State, mini-barre de progression
   + % + temps restant si Printing/Paused. Même use-case et même mécanique de
   fragment/poll que les cartes. Chaque ligne pointe vers la vue Imprimantes.
6. **Instance démo** — Machine Link désactivée quand l'instance est en mode
   démo : section de formulaire absente, panneau Farm Activity absent.
7. **i18n** — tous les libellés (états, section formulaire, panneau) passent
   par des clés i18n fr/en. A11y : contrastes AA, pas d'animation de barre
   (`prefers-reduced-motion`), fragments compatibles axe.

**Key interfaces:** (glossaire : Printer, Printer Brand, Machine Link, Machine Status, Machine State, Farm Activity)
- `MachineLink` — nouveau concept domaine attaché à une Printer : variantes
  PrusaLink (host + clé API) et Moonraker (URL). Persisté avec la Printer,
  credentials chiffrés (ADR-0006).
- `MachineStatus` — valeur domaine : Machine State + progression optionnelle
  (%, temps restant) + nom de job optionnel + températures optionnelles.
  Jamais persistée.
- Port API : `get_printer_status(printer_id) → MachineStatus` (Offline si
  injoignable) et `test_machine_link(link) → résultat de test`.
- Port SPI `MachineStatusProbe` : `fetch_status(&MachineLink) → MachineStatus`
  — deux adapters REST (PrusaLink `/api/v1/status` + `/api/v1/job` ;
  Moonraker `/printer/objects/query`) dans la couche infrastructure, timeout
  court, aucune redirection suivie.
- Le domaine reste pur : aucun HTTP/crypto dans `domain` ; chiffrement dans la
  couche persistance, HTTP dans les adapters SPI.

**Acceptance criteria:**
- [ ] Prusa avec hôte+clé valides : la carte affiche badge + job + températures conformes à la machine ; le dashboard liste la machine dans Farm Activity.
- [ ] Other avec toggle Klipper + URL Moonraker valide : idem via Moonraker.
- [ ] Other sans toggle Klipper : aucune section connexion, aucun appel réseau, comportement actuel inchangé.
- [ ] Machine configurée mais éteinte/injoignable : badge Offline sous ~3 s, page et fragment rendus sans erreur.
- [ ] Printer sans Machine Link : absente du panneau Farm Activity ; carte sans badge d'état ni bloc job.
- [ ] « Tester la connexion » : succès affiche l'état détecté ; échec (mauvaise clé, hôte injoignable) affiche une erreur claire, sans soumettre le formulaire.
- [ ] La clé API est chiffrée en DB (la valeur en clair n'apparaît dans aucune colonne) ; l'édition montre « configuré », jamais la valeur ; aucun credential dans le HTML ni les fragments.
- [ ] Boot avec Machine Link existante et `FILATURE_CREDENTIALS_KEY` absente/invalide : échec explicite au démarrage (pas de dégradation silencieuse).
- [ ] Badge d'occupation (21) et badge d'état live coexistent dans le header de carte.
- [ ] Mode démo : ni section connexion, ni panneau Farm Activity.
- [ ] Libellés i18n fr/en ; e2e a11y (Playwright + axe) sans violation ; contraste bloquant vert ; `prefers-reduced-motion` respecté.
- [ ] Tests : domaine (mapping états, repli tête active→première tête), adapters SPI contre serveurs HTTP factices (statuts nominaux, timeout, erreur auth), fragment htmx. Suite complète verte.

**Out of scope:**
- Bambu Lab (MQTT) — slice `22b`.
- Synchro des bobines chargées depuis la machine — slice `23` (notée).
- Contrôle machine (pause/stop/lancement), caméra, historique, notifications.
- Clé API Moonraker (décision : URL seule ; champ ajouté plus tard si besoin).
- Poller d'arrière-plan / cache de statut en DB — modèle proxy à la demande uniquement.
- Températures de toutes les têtes (décision : tête active, repli première).

**References:**
- Brief produit : `docs/product/brief.md` (job #5)
- ADR chiffrement : `docs/adr/0006-machine-credentials-encryption.md`
- Glossaire : `docs/glossary.md` (§ Machine connectivity)
- Design/UI validé : review lavish du 2026-07-22 (`.lavish/machine-link-design.html`, non committé)
- Specs amont : `docs/specs/15a…17-printer-*.md`, `21-printer-cards-layout.md`, `05-dashboard.md`
