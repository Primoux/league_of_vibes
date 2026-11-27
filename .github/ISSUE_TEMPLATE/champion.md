---
name: Champion
about: Ajouter un champion de bout en bout (la tâche la plus parallélisable du projet)
title: "[champion] "
labels: ["area:champion"]
---

<!-- Un champion = une tâche = une PR = quatre fichiers qui n'appartiennent qu'à lui.
     C'est le découpage qui permet à plusieurs personnes de livrer plusieurs champions
     en parallèle sans un seul conflit. -->

## Identité
| | |
|---|---|
| Nom | |
| Identifiant (minuscules, sans accent) | |
| Titre | |
| Classe | <!-- tank, combattant, mage, assassin, tireur, support --> |
| Voies | |
| Silhouette et palette | <!-- deux ou trois phrases : on doit pouvoir le reconnaître à la silhouette --> |

## Kit
| Touche | Nom | Effet |
|---|---|---|
| P | | |
| Q | | |
| W | | |
| E | | |
| R | | |

Chiffres (dégâts, coûts, recharges, portées) : à caler sur un champion existant de la même classe dans
`crates/lov-shared/src/data/champions/`, puis à ajuster.

## Les quatre fichiers à écrire
- [ ] `crates/lov-shared/src/data/champions/<id>.rs` — stats, sorts, coûts
- [ ] `crates/lov-sim/src/abilities/champions/<id>.rs` — logique serveur
- [ ] `crates/lov-client/src/render/champions/models/<id>.rs` — modèle 3D
- [ ] `crates/lov-client/src/fx/champions/<id>.rs` — FX
- [ ] une ligne ajoutée dans chaque `index.ts` de registre
- [ ] un test headless du kit dans `crates/lov-sim/tests/`

## Contrat partagé nécessaire ?
<!-- Un kit qui demande un nouveau type d'effet, de CC ou de message réseau touche un contrat partagé :
     ouvre d'abord une issue « contrat » et fais-la valider. Sinon « Aucun ». -->
Aucun

## Comment on saura que c'est fait
- `?modellab&champ=<id>` : modèle et animations corrects.
- `?fxlab&champ=<id>&close=1` : les cinq sorts réellement simulés.
- Le test headless du kit passe.
- Aucun nom, visuel, son ni texte de Riot. Textes en français.
