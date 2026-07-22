# Handoff : Filature — gestionnaire de stock de filament 3D (UI)

## Overview
Filature est un outil auto-hébergé de suivi de stock de filament 3D (usage perso + micro-entreprise « Zig Factory »). C'est un **instrument d'atelier** qu'on garde ouvert dans un onglet, pas une app grand public. Cette maquette couvre les 7 écrans clés : Tableau de bord, Bobines (liste), Humidité (dryboxes), Détail bobine, Formulaire ajout/édition, Référentiel matériaux, Paramètres (config d'instance + référentiels Fabricants/Emplacements).

## About the Design Files
Le fichier `Filature.dc.html` est une **référence de design réalisée en HTML** — un prototype qui montre l'apparence et le comportement voulus, **pas du code de production à copier tel quel**. Il est écrit dans un mini-framework interne (balises `<x-dc>`, `<sc-for>`, `<sc-if>`, holes `{{ }}`, classe `Component extends DCLogic`) qui **n'est pas** la cible d'implémentation.

**Cible d'implémentation prévue** (contrainte forte du projet) : **HTML server-rendered + CSS vanilla + htmx**, servi par un **binaire Rust unique**, **sans framework JS front, sans build front**. Recréez ces écrans dans cet environnement. Le design a été pensé pour ça :
- Chaque unité qui se met à jour seule (une ligne de bobine, une carte humidité, le panneau liste) doit être un **fragment htmx** autonome re-rendu en place (swap).
- Interactions réalistes visées : filtrage qui remplace la liste (`hx-get` → swap), édition inline du poids qui remplace la ligne, humidité rafraîchie périodiquement (`hx-trigger="every 60s"`).
- Pas de SPA, pas de drag-and-drop, au plus une modale simple.
- Icônes : jeu léger type Lucide/Feather (celles du proto sont des SVG Feather inline).

Ne portez pas la logique JS du proto (state en mémoire, `setState`) ; elle simule le back. En Rust/htmx, l'état vit côté serveur et les handlers renvoient des fragments HTML.

## Fidelity
**High-fidelity (hifi).** Couleurs, typographie, espacements et interactions sont définitifs. Recréez l'UI au pixel près. Les tokens exacts sont en fin de doc.

---

## Règles non négociables (colonne vertébrale du design)
1. **Thèmes clair ET sombre à parité.** Défaut = préférence OS (`prefers-color-scheme`), plus une bascule manuelle persistante. Implémentation : tokens CSS sur `:root`, override via `html[data-theme="light|dark"]`, persistance (cookie/localStorage → attribut `data-theme` sur `<html>`, posé au rendu serveur).
2. **Base neutre, légèrement chaude** (gris atelier, pas de bleu-nuit clinique). 3 niveaux de surface pour hiérarchiser.
3. **La seule couleur vive vient des données.** Chaque bobine porte sa **couleur réelle de filament** (pastille ronde + liseré de carte/ligne). C'est la seule source de couleur « libre » à l'écran.
4. **Sémantique réservée au statut** : vert = OK, ambre = à surveiller, rouge = à sécher / stock bas. Jamais en décoration. Une variante par thème (les teintes dark sont plus claires pour ne pas baver).
5. **Monospace pour tous les chiffres et unités** (poids g, longueur m, %, %HR, °C, prix €) — aligne les colonnes, renforce le côté instrument. Sans-serif technique pour le reste de l'UI.

---

## Design Tokens

### Typographie
- **UI (sans-serif)** : `IBM Plex Sans`, fallback `system-ui, sans-serif`. Poids 400/500/600/700.
- **Chiffres & unités (mono)** : `IBM Plex Mono`, fallback `ui-monospace, monospace`. Poids 400/500/600.
- Tailles courantes : titres écran 19px/600 ; H2 section 14px/600 ; corps 13–14px ; labels mono en capitales 10–11px avec `letter-spacing:.06–.08em`, `text-transform:uppercase`, couleur « faint » ; grands chiffres KPI 27px/600 mono ; %HR géant (carte humidité) 38px/600 mono ; reste bobine (détail) 40px/600 mono.

### Couleurs — thème CLAIR
```
--bg:#ece8e1   --surface:#f6f3ee   --raised:#fffefb        (3 niveaux de surface, chaud)
--border:#e2dcd0   --border-strong:#cec6b7
--text:#282520   --muted:#6d675c   --faint:#9b9284
--active-bg:#e4ded2   --hover-bg:#ebe5da
--accent:#5b5563 (slate quasi-neutre ; défaut retenu)   --accent-fg:#ffffff
--ok:#3f7d4e    --ok-bg:#e3efe3
--warn:#a06f1c  --warn-bg:#f2e8d6
--danger:#b23a2e --danger-bg:#f3e0da
--shadow:0 1px 2px rgba(40,35,25,.05), 0 1px 1px rgba(40,35,25,.03)
```
### Couleurs — thème SOMBRE
```
--bg:#211f1c   --surface:#282521   --raised:#302d28
--border:#39352f   --border-strong:#4a453d
--text:#ece7df   --muted:#a49c8f   --faint:#736c60
--active-bg:#332f29   --hover-bg:#38342d
--accent:#6b7686 (slate plus clair sur fond sombre)   --accent-fg:#ffffff
--ok:#5aa86b    --ok-bg:#25352a
--warn:#c9922f  --warn-bg:#38301f
--danger:#d6584a --danger-bg:#3a2723
--shadow:0 1px 2px rgba(0,0,0,.28), 0 1px 1px rgba(0,0,0,.2)
```
> L'accent est aussi proposé en 4 variantes curées (`#5b5563` slate défaut, `#4a5568`, `#4a6b52` vert sapin, `#8a5a2b` cuivre) — un seul accent discret à la fois.

### Rayons, espacements
- Rayons : cartes/panneaux 11–12px ; boutons/inputs/badges 6–8px ; pastilles = cercle ; pills de statut 20px.
- Bordures : 1px `--border` (séparations), `--border-strong` (contours d'input/badge et anneau de chip).
- Espacements récurrents : gap de grille 14px ; padding carte 15–22px ; padding écran horizontal `--pad-x` (28px desktop → 15px mobile) ; hauteur de barre-jauge 7–11px.

### Chip couleur filament (composant transverse)
- Cercle avec **toujours** `border:1px solid var(--border-strong)` (indispensable pour blanc/noir sur fond clair comme sombre).
- Couleur = hex réel de la bobine en `background`.
- **Transparent** : pas de couleur pleine mais une hachure `repeating-linear-gradient(45deg, var(--border-strong) 0 2px, transparent 2px 5px)` ; le liseré de carte/ligne retombe sur un gris neutre `#9aa0a6`.
- Liseré de ligne/carte = même hex (bord gauche 3px ; 4px sur la carte de détail).

### Jauge de poids restant (composant signature)
- **Barre horizontale** : piste `--active-bg`, remplissage à `width = %restant`.
- Couleur du remplissage : **neutre** (`--muted`) en régime normal → **ambre** (`--warn`) sous le seuil bas → **rouge** (`--danger`) sous 10 % → gris (`--border-strong`) si vide. Le texte g/% prend la même couleur sémantique quand bas.
- Toujours accompagnée de la valeur en **g** et du **%** en mono.

---

## Screens / Views

### Navigation (shell, présent partout)
- **Sidebar gauche**, largeur 216px, `background:--surface`, bord droit 1px.
- Wordmark en haut : petit pictogramme « bobine » (2 cercles concentriques + 4 tirets d'axe) + `FILATURE` en mono 15px, `letter-spacing:.16em`.
- 5 items : **Tableau de bord**, **Bobines** (badge compteur à droite), **Humidité** (pastille rouge si alerte), **Matériaux**, puis un espace flexible qui repousse **Paramètres** en bas du groupe de nav (juste au-dessus de la bascule de thème). Item actif : fond `--active-bg`, texte `--text` ; inactif : texte `--muted`, icône Feather 17px.
- Bas de sidebar : bascule de thème (segmenté Auto / Clair / Sombre) + ligne « Zig Factory · auto-hébergé ».
- **Décision d'IA (juillet 2026)** : Fabricants et Emplacements ne sont **pas** des items de nav de premier niveau — ce sont des données de référence saisies une fois puis rarement modifiées. Elles vivent en sous-onglets de l'écran **Paramètres**. Matériaux, lui, reste en nav principale (consulté au quotidien : statuts, seuils d'humidité, alertes).
- Détail et Formulaire ne sont pas des items de nav (on y accède depuis Bobines) mais gardent l'item **Bobines** actif.

### 1. Tableau de bord
- **Purpose** : état du stock en un coup d'œil ; alertes actionnables ensuite.
- **Layout** : header (titre + date + bouton primaire « Ajouter une bobine »). Contenu scrollable :
  1. **4 cartes KPI** (grille 4 colonnes) : Valeur du stock (€), Poids restant (kg), Bobines (total + actives/vides), **Alertes** (carte en `--danger-bg`/bord `--danger`, chiffre rouge).
  2. Grille 2 col (`1.05fr .95fr`) : **Répartition par matériau** (une ligne par matériau : nom mono, nb bobines, poids kg, mini-barre neutre proportionnelle au max) · **Bientôt vides** (liste courte : chip + nom + matériau/rangement + reste g/% coloré si bas).
  3. **Humidité des dryboxes** : 3 cartes compactes (nom, %HR géant coloré, °C, pastille de statut, note). Lien « Tout voir → ».
- KPIs calculés : valeur = Σ(reste/net × prix) ; poids = Σreste ; alertes = (bobines sous seuil) + (dryboxes en rouge).

### 2. Bobines (écran de travail principal)
- **Purpose** : liste dense filtrable/triable, éditable en place, **sans rechargement** (chaque filtre remplace la liste → fragment htmx).
- **Barre de filtres** (sticky, `--surface`) : recherche texte + 4 selects (matériau, marque, statut, rangement) + « Réinitialiser » (visible si un filtre actif). En htmx : chaque contrôle `hx-get` renvoie le `<tbody>`/la grille.
- **Deux vues au choix** (toggle Table / Cartes dans le header) :
  - **Table dense** : colonnes Bobine (chip + marque/couleur) · Matériau (badge mono outline neutre) · **Reste** (jauge + g + % + icône crayon) · Rangement (mono) · Statut (pastille + label). Ligne cliquable → détail. Liseré gauche 3px = couleur filament. Densité réglable (comfortable/compact = padding vertical 11px/7px).
  - **Grille de cartes** : carte par bobine, liseré gauche couleur, chip + marque/couleur + badge matériau en tête, gros reste g + %/net, barre-jauge, pied rangement + statut. `minmax(min(248px,100%),1fr)`.
- **Édition inline du poids** : clic sur la cellule Reste (ou le crayon) → remplace l'affichage par `input (g) + Enregistrer + ✕`. Entrée = valider, Échap = annuler. En htmx : `hx-get` du form de ligne puis `hx-put` qui renvoie la ligne re-rendue. Passer le reste à 0 bascule le statut en « Vide ».
- **État vide** : si aucun résultat, message + bouton « Réinitialiser les filtres ».
- Statuts : `sealed`→Scellée (pastille verte), `open`→Ouverte (pastille neutre), `empty`→Vide (rouge), `archived`→Archivée.

### 3. Humidité (le différenciateur)
- **Purpose** : surveiller chaque drybox ; alerter si un matériau sensible dépasse son seuil. Doit rester **impeccable et lisible sur mobile** (consulté depuis le téléphone).
- **Layout** : grille de cartes `minmax(min(330px,100%),1fr)`. Header : « N dryboxes surveillées · rafraîchit auto toutes les 60 s » + « capteurs en ligne ». En htmx : chaque carte `hx-trigger="every 60s"` se re-render seule.
- **Carte drybox** : bord haut 3px coloré selon le pire statut. Contenu : nom + localisation + pill de statut ; **%HR géant** et **°C géant** (mono, couleur = statut) ; **sparkline** 24 h (polyline SVG, couleur = statut) + min–max. Séparateur, puis **liste des matériaux rangés ici** : nom mono, sensibilité (Low/Medium/High), seuil %HR, et **statut par matériau** (Stable / À surveiller / À sécher) — la ligne passe en fond ambre/rouge si au-dessus du seuil.
- **Règle d'alerte** : seuil %HR par sensibilité — Low=40, Medium=30, High=15. `%HR > seuil` → rouge « À sécher » (label « À sécher » si High, sinon « Trop humide ») ; `> seuil−6` → ambre « À surveiller » ; sinon vert « Stable ». Le statut de la carte = le pire de ses matériaux. (Scénario type : nylon PA-CF/PA-GF dans une drybox à 41 %HR → rouge.)
- Note de bas d'écran : les étagères ouvertes ne sont pas sondées ; seuls les matériaux sensibles vont en drybox.

### 4. Détail bobine
- **Purpose** : toutes les infos + actions.
- **Layout** : header (retour « Bobines » + actions **Éditer** et **Archiver**). Colonne centrée max 840px :
  - **Carte héro** (liseré gauche 4px couleur) : chip 34px + marque + couleur + badge matériau + statut ; **reste géant en g + longueur en m + %** ; barre-jauge ; « sur {net} g · {longueur totale} m ». Bouton **Ajuster le poids** → édition inline (même mécanisme que la liste, reste sur l'écran).
  - **Grille d'infos 3×3** : Diamètre, Poids net initial, Valeur actuelle, Prix payé, Rangement, Fournisseur, Acheté le, Ouvert le, Statut (valeurs en mono).
  - **Carte humidité du rangement** : si le rangement est une drybox → %HR/°C + statut ; sinon note « non sondé (étagère ouverte) ».
  - **Notes** (si présentes).
  - **Historique de consommation** : état « à venir » (le suivi par impression arrivera plus tard — ne pas inventer de données).
- **Longueur (m)** calculée depuis la masse : `L_m = (masse_g / densité) / (π·(d/2/10)²) / 100` (d en mm ; ex. Ø1.75). La densité vient du référentiel matériaux.

### 5. Formulaire ajout / édition
- **Purpose** : saisie rapide, geste répété. Ouvert par les boutons « Ajouter » (mode add) ou « Éditer » du détail (mode edit, prérempli).
- **Layout** : header (titre + Annuler / Enregistrer). Colonne max 680px, 3 cartes-sections :
  - **Identité** : select Matériau, input Marque, **Couleur** — pas de champ « nom de couleur » saisi par l'utilisateur : le nom est **dérivé automatiquement du hex** (nom de la pastille préréglée si ça matche, sinon le code hex lui-même, ex. « #C62828 »), et c'est cette valeur dérivée qui est stockée. UI : une **grille de pastilles préréglées** (avec libellé visible sous chacune, dont « Transparent »), puis sous un séparateur « ou personnalisée » : une pastille de prévisualisation qui est aussi un vrai `<input type="color">` caché (clic → sélecteur système, badge crayon en overlay) + un champ hex texte (`#RRGGBB`, validation/normalisation au blur — accepte aussi le format court `#RGB` et le préfixe `#` auto-ajouté), avec libellé dérivé affiché sous le champ et message d'erreur inline si le hex est invalide.
  - **Mesures** : Diamètre en segmenté (1.75 / 2.85 mm) ; input **Poids net filament (g)** ; encart **« Ou peser la bobine → poids net déduit »** : `Poids total pesé (g)` − `Tare bobine vide (g)` = **Net filament** (recalculé en direct, affiché en gros accent). C'est le cas d'usage clé « je pèse → tare auto → net déduit ».
  - **Rangement & achat** : select Rangement, Prix (€), dates Acheté le / Ouvert le, Notes (textarea).
- À l'enregistrement : add → crée la bobine (reste = net, statut Scellée) puis ouvre son détail ; edit → met à jour puis revient au détail.

### 6bis. Paramètres
- **Purpose** : configuration d'instance + gestion des référentiels peu volatils (Fabricants, Emplacements) + sauvegarde/restauration.
- **Layout** : header titre + barre de 4 onglets (**Général**, **Fabricants**, **Emplacements**, **Sauvegarde**), soulignement 2px `--accent` sur l'onglet actif, le reste en `--muted`. Contenu scrollable en dessous.
  - **Général** : carte « Stock » — input **Seuil de stock bas** (%) + bouton **Enregistrer**. Même sémantique que `lowStockPct` (défaut 15) qui pilote déjà le calcul « bientôt vide » partout ailleurs dans l'app.
  - **Fabricants** : carte liste — header avec compteur + input texte « Nom du fabricant » et bouton **+ Ajouter** ; une ligne par fabricant (nom, nombre de bobines qui le référencent en mono, icône poubelle pour retirer).
  - **Emplacements** : même patron que Fabricants (nom du rangement, nb de bobines qui l'utilisent, ajout/suppression).
  - **Sauvegarde** : deux colonnes — **Exporter** (bouton qui télécharge un JSON complet : bobines, matériaux, fabricants, emplacements, réglages) ; **Importer** (file input JSON, limite 1 Mio, case à cocher de confirmation obligatoire, bouton rouge **Remplacer l'instance** désactivé tant que la case n'est pas cochée et qu'aucun fichier n'est choisi — remplace tout, ne fusionne rien).
- **Note produit** : Fabricants/Emplacements n'ont aujourd'hui aucune contrainte d'intégrité référentielle forte avec les bobines dans le proto (suppression libre même si des bobines y font encore référence) — à trancher côté implémentation réelle (bloquer, avertir, ou détacher silencieusement).

### 6. Référentiel matériaux
- **Purpose** : écran de config, moins fréquent. **Source unique** des valeurs par défaut : modifier une sensibilité met à jour partout les seuils d'humidité et les longueurs (via la densité).
- **Layout** : une **table éditable**, une ligne par matériau. Colonnes : Matériau (badge), **Densité** g/cm³ (input), **Séchage** (temp °C + temps h, inputs), **Sensibilité** (select Low/Medium/High, texte coloré vert/ambre/rouge), **Seuil %HR** (dérivé, lecture seule), **Buse** déf. °C (input), **Plateau** déf. °C (input). Les inputs sont discrets (fond `--surface`, bord transparent → accent au focus).

---

## Interactions & Behavior (résumé pour htmx)
- **Navigation** : sidebar change d'écran ; ligne/carte bobine → détail ; boutons Ajouter → formulaire ; retour/annuler → liste ou détail.
- **Filtrage** : chaque contrôle de la barre → GET qui renvoie la liste filtrée (remplace `<tbody>` ou la grille). Debounce léger sur la recherche.
- **Édition inline poids** : GET fragment d'édition (remplace la cellule/ligne) → PUT (renvoie la ligne). Entrée valide, Échap annule.
- **Humidité** : polling 60 s par carte (`hx-trigger="every 60s"`), swap de la carte. Le statut se recalcule côté serveur.
- **Thème** : bouton pose un cookie + attribut `data-theme` sur `<html>` (rendu serveur au chargement pour respecter l'OS par défaut).
- **Archiver** : action sur le détail ; une confirmation simple est souhaitable (non implémentée dans le proto).

## Responsive
Priorité desktop. Bascules pilotées par variables CSS aux points de rupture :
- **≤ 1040px (tablette)** : KPIs en 2 colonnes ; sections dashboard empilées ; résumé dryboxes en 2 colonnes.
- **≤ 760px (mobile)** : sidebar → **rail d'icônes** (60px, labels masqués, bascule thème masquée — thème suit l'OS) ; tout en 1 colonne (KPIs, dryboxes, grille d'infos détail, formulaire) ; padding écran réduit (15px) ; table des bobines scrollable horizontalement (`min-width` + `overflow-x:auto`). Dashboard et Humidité restent pleinement lisibles (exigence).
Dans le proto, ces bascules passent par des vars (`--sidebar-w`, `--nav-label`, `--kpi-cols`, `--dash-cols`, `--dry-cols`, `--detail-cols`, `--form-cols`, `--pad-x`) flippées par `@media` — reproduire avec de vraies media queries CSS.

## State Management (côté serveur en Rust)
- Entités : **Bobine** (marque, matériau, couleur+hex, reste_g, net_g, statut, rangement, diamètre, prix, dates achat/ouverture, fournisseur, notes) ; **Drybox/capteur** (nom, localisation, %HR, °C, historique) ; **Matériau** (densité, temp/temps séchage, sensibilité, buse/plateau déf.).
- Dérivés recalculés : %restant, longueur m, valeur, statut de bobine, seuil %HR (depuis sensibilité), statut de drybox (pire matériau), compteurs/alertes du dashboard.
- Le référentiel matériaux est la source de vérité pour densité + sensibilité.

## Assets
- **Polices** : IBM Plex Sans + IBM Plex Mono (Google Fonts, poids listés ci-dessus). Auto-héberger les woff2 pour un binaire sans dépendance réseau.
- **Icônes** : Feather/Lucide (nav, actions, crayon d'édition, gouttes, alerte). Inline SVG dans le proto — remplacer par le jeu d'icônes du projet.
- **Pas d'images** : aucune illustration ; le seul « visuel » est la couleur des données.

## Files
- `Filature.dc.html` — le prototype hifi complet (6 écrans, clair/sombre, responsive). Ouvrable dans un navigateur avec `support.js` à côté ; **référence visuelle uniquement**.
- `support.js` — runtime du proto (non pertinent pour l'implémentation cible).
