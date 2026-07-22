# Brief : refonte des cartes Imprimantes 3D + passage px → rem

## 1. Refonte des cartes de l'écran Imprimantes 3D

### Problème initial
La grille de cartes (`grid-template-columns: repeat(auto-fill, minmax(340px,1fr))`) forçait chaque carte à s'étirer à la hauteur de la plus haute carte de sa ligne (comportement par défaut `align-items:stretch` de CSS grid). Résultat : les imprimantes à une seule bobine (Core One, X-Max) se retrouvaient avec une grande zone blanche vide en bas de carte à côté d'une imprimante AMS (P1S, X1C) plus riche en contenu. Second problème : à l'intérieur d'une carte AMS, les chips de bobine avaient une largeur fixe de 158px — sur une carte plus large (colonne masonry ou grande fenêtre), ça laissait une bande blanche verticale à droite du dernier chip.

### Solution implémentée
1. **Layout en masonry (CSS multi-colonnes)** au lieu d'une grid stricte : `columns:340px; column-gap:16px` sur le conteneur, `break-inside:avoid; margin-bottom:16px` sur chaque carte. Chaque carte garde sa hauteur naturelle (pas d'étirement forcé) et s'empile en colonnes façon Pinterest — plus de zone blanche interne.
2. **Chips AMS en flex fluide** : `width:158px` fixe remplacé par `flex:1 1 140px; min-width:140px` sur les chips pleins et vides. Ils s'étirent pour occuper toute la largeur disponible de la carte, quelle que soit sa largeur réelle.
3. **Badge d'occupation dans le header de carte** : ajout d'une pastille `"{filled} / {total} chargées"` à côté du bouton éditer, colorée sémantiquement (vert = tout chargé, ambre = partiellement chargé, neutre = tout vide). Donne un repère cohérent identique sur toutes les cartes, quel que soit le nombre de groupes/bobines.
4. **Label de groupe systématique** : les groupes à une seule bobine (ex. "Bobine externe", "Bobine") affichent désormais leur label au-dessus du slot, comme les groupes multi-bobines (AMS) — auparavant ce label n'apparaissait que sur les slots vides. Unifie le langage visuel entre carte mono-bobine et carte multi-bobines (AMS).

### À reproduire dans l'implémentation cible (Rust/htmx)
- Conteneur de cartes en CSS multi-colonnes (`column-width` ou `columns`), pas de grid avec stretch implicite.
- Chaque carte imprimante = fragment htmx autonome (assignation/retrait de bobine → re-render de la carte ou du slot uniquement).
- Chips de bobine en flex (`flex:1 1 140px`), jamais en largeur fixe.
- Calculer `filled/total` côté serveur pour le badge d'occupation (même logique que le calcul de statut "bientôt vide" du reste de l'app).

## 2. Unités CSS : rester en px (décision, pas un TODO)

Question posée : passer les valeurs en `em`/`rem` plutôt que `px`.

**Décision : garder px.** Ce n'est pas un renoncement technique mais un choix cohérent avec le reste du design : c'est une mise en page à dimensions fixes (comme un instrument d'atelier), pas du contenu éditorial destiné à re-scaler avec la taille de police du navigateur. Tous les tokens du design (rayons, espacements, tailles de police, largeurs de colonnes) sont en px dans le prototype — à reproduire tel quel en CSS px dans l'implémentation, sauf si un besoin d'accessibilité précis (zoom utilisateur, préférence de taille de police système) apparaît en aval, auquel cas re-évaluer au cas par cas (typiquement : passer `font-size` en `rem` en gardant les autres dimensions en `px`).
