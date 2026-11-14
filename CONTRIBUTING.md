# Comment bosser sur League of Vibes

Ce fichier est le mode d'emploi : tu le lis une fois, tu l'as compris, tu peux livrer.
Les **règles** sont dans [`AGENTS.md`](AGENTS.md), le **design du jeu** dans [`PLAN.md`](PLAN.md).
Ici, c'est la marche à suivre.

On est plusieurs, chacun sur sa machine, chacun avec ses propres IA. Tout tient sur deux idées :

> **Une issue = une tâche = une branche = une PR.**
> **Une PR ne touche qu'une seule *area*.**

Le reste n'est que de l'outillage pour rendre ces deux phrases vraies.

---

## 1. La boucle, en entier

```
   issue libre  →  tu t'assignes  →  branche  →  code  →  PR  →  relecture  →  merge  →  branche supprimée
        ↑                                                                                      │
        └──────────────────────────── tu prends la suivante ───────────────────────────────────┘
```

Une boucle dure de quelques heures à un jour. **Jamais plus** : une branche qui traîne prend du retard sur
`main`, conflicte, et finit à la poubelle. On a déjà perdu des branches à 231 commits de retard.

---

## 2. Jour 1 : préparer ta machine

```bash
git clone git@github.com:Primoux/league_of_vibes.git
cd league_of_vibes
scripts/setup.sh <ton-pseudo>      # ex. scripts/setup.sh enzo
```

Le script écrit `.agent` (non versionné : il dit à tes IA qui elles sont sur cette machine), règle
`git config user.name` — sinon toutes les contributions se ressemblent dans l'historique —, installe et teste.

Vérifie la version de Node : `.nvmrc` dit **24**, comme la CI. Avec une autre version, tu verras des échecs
que personne d'autre ne voit.

Puis lance le jeu :

```bash
npm run dev          # serveur sur :3000, client sur :5173
```

Ouvre <http://localhost:5173/?dev=1> dans **deux onglets** : crée une salle dans le premier, rejoins-la avec
le code dans le second, lance la partie. `?dev=1` autorise une salle de moins de 8 joueurs.

Raccourci utile quand tu veux juste voir le jeu tourner :

```bash
SKIP_DRAFT=1 DEV_LOBBY=1 npm run dev:server     # pas de draft, champions attribués d'office
```

Et pour travailler **sans serveur du tout** (`npm run dev:client` suffit) :

| URL | Ce que tu vois |
|---|---|
| `?fxlab&champ=nyx` | les sorts d'un champion, pilotés par la vraie simulation |
| `?modellab&champ=nyx` | son modèle 3D et ses animations, à côté du portrait de référence |
| `?maplab` | la carte seule |
| `?uilab=home` / `=draft` / `=game` | les écrans et le HUD avec des données factices |

---

## 3. Prendre une tâche

Le tableau de bord, c'est les **Issues GitHub** : c'est le seul endroit visible depuis toutes les machines.

```bash
gh issue list --state open                        # tout ce qui est ouvert
gh issue list --label "bonne première tâche"      # pour commencer
gh issue list --label area:fx-core                # par domaine
gh issue edit 57 --add-assignee @me               # ← PRENDRE la tâche
```

**S'assigner une issue, c'est poser un verrou sur ses fichiers.** C'est ce qui remplace « tel dossier
appartient à telle personne » : tant qu'une issue est assignée à quelqu'un d'autre, tu ne touches pas à son
périmètre. Deux personnes ne peuvent pas prendre la même issue, et ça marche entre machines.

- Rien ne te va ? Ouvre une issue avec le modèle fourni, puis assigne-la toi.
- Tu abandonnes, ou tu n'avances plus ? **Désassigne-toi tout de suite** pour libérer le verrou. Ce n'est pas
  un échec, c'est la seule chose à faire pour ne pas bloquer les autres.
- Si tu oublies : une issue assignée sans activité pendant **14 jours** est désassignée automatiquement,
  avec un commentaire. Rien n'est perdu — tu te la réassignes si tu es toujours dessus. Mais préfère ouvrir
  une PR en brouillon que de garder du travail commencé sur ta machine.

---

## 4. Trouver son périmètre : les *areas*

[`.github/areas.json`](.github/areas.json) découpe le projet en **areas** : chacune liste les chemins qu'elle
couvre. `net`, `nav`, `combat`, `abilities`, `items`, `systems`, `world-ai`, `render`, `render-map`,
`render-champions`, `fx-core`, `fx-effects`, `audio`, `ui-screens`, `ui-hud`, `ui-core`, `input`,
`client-net`, `champion`, `infra`, `docs`.

**Le nom de ton area est aussi le scope de ton titre de PR** : `feat(ui-hud): …`, `fix(nav): …`.

Avant de pousser, vérifie que tu es dans les clous :

```bash
node scripts/check-areas.mjs --title "feat(ui-hud): barre de mana"
```

La CI refait exactement ce test.

Si ta tâche déborde sur deux areas, demande-toi d'abord si ce sont **deux issues et deux PR** : c'est presque
toujours la bonne réponse, et c'est ce qui permet aux relectures d'avancer en parallèle.

Mais certaines fonctionnalités traversent vraiment le code — un réglage qui demande un champ dans le store,
un bouton dans le HUD et une touche dans l'input. Dans ce cas tu **déclares** les areas concernées, dans le
titre ou dans le corps de la PR :

```
feat(input,ui-hud): mode plein écran
```
```
Areas: input, ui-hud, client-net
```

Au-delà de trois areas la CI t'avertit sans bloquer : c'est le signe d'une PR trop large, mais c'est toi qui
juges si les morceaux tiendraient debout séparément. Déclarer une area, c'est prendre le verrou dessus :
n'en déclare pas « au cas où ».

### Le cas idéal : un champion de bout en bout

Le code range **un fichier par champion** à chaque étage. Tu peux donc livrer un champion complet sans
croiser personne :

```
packages/shared/src/data/champions/<nom>.ts            stats, sorts, coûts
packages/server/src/abilities/champions/<nom>.ts       logique serveur
packages/client/src/render/champions/models/<nom>.ts   modèle 3D
packages/client/src/fx/champions/<nom>.ts              FX et sons
```

Titre : `feat(champion): <Nom>`. Une PR `champion` ne couvre **qu'un** champion, la CI le vérifie. Les seuls
fichiers partagés sont les `index.ts` de registre, où tu ajoutes une ligne.

C'est le découpage à privilégier dès que c'est possible : vingt personnes peuvent livrer vingt champions en
parallèle sans un seul conflit.

---

## 5. Coder

```bash
git fetch origin
git switch -c ui-hud/barre-de-mana origin/main      # <area>/<slug>, toujours depuis un main à jour
```

Pendant le travail :

- **Ne va pas éditer le code d'un autre domaine pour y brancher ta logique.** Passe par les points de
  rendez-vous prévus : côté serveur l'`EventBus` (`world.events.on/emit`), `entity.data.<domaine>` et les API
  de `PLAN.md §2.1` (`applyDamage`, `addBuff`…) ; côté client le `store` (`client/src/state.ts`) et le
  `fxBus` (`client/src/fx/FxBus.ts`).
- **Une API dont tu dépends n'existe pas encore ?** Mocke-la chez toi, ou crée-la avec la signature exacte du
  `PLAN.md` et une implémentation minimale — et dis-le dans ta PR. N'attends pas.
- **Toute règle de jeu vient avec son test** headless (`packages/*/test/`) : crée un `World`, ajoute des
  entités, fais des `step()`, vérifie.
- **Tu ne touches jamais au test d'un autre** pour faire passer ta PR. Si un test d'autrui casse à cause de
  toi, c'est ton code — ou le contrat — qui doit changer. Dis-le dans la PR.
- Commits petits et fréquents, en conventionnel : `feat(ui-hud): …`, `fix(nav): …`, `test: …`, `docs: …`.

Avant de pousser, obligatoire et vert :

```bash
npm run typecheck && npm test && npm run build
```

---

## 6. Ouvrir la PR

```bash
git push -u origin ui-hud/barre-de-mana
gh pr create --fill          # puis remplis le template
```

Le titre porte l'area. Le corps référence l'issue (`Closes #57`). Trois jobs tournent :

| Job | Ce qu'il vérifie |
|---|---|
| `check` | typecheck, tests, build du client, puis 8 bots WebSocket qui jouent vraiment |
| `areas` | ta PR reste dans son area, ne touche aucun contrat partagé, et ne couvre qu'un champion |
| `size` | avertit au-delà de ~800 lignes modifiées |

Le job `areas` pose aussi le label `area:<ton-area>` sur la PR, pour qu'on filtre la file comme les issues.

**CI rouge = la PR ne sera pas relue.** Corrige sur ta branche et repousse : la PR se met à jour toute seule.

`main` a bougé pendant ce temps ?

```bash
git fetch origin && git merge origin/main      # puis repousse
```

**Tu ne merges jamais ta propre PR.** Un mainteneur relit, valide et merge en `--squash` : ta tâche devient un
commit sur `main`. Pendant l'attente, tu prends la tâche suivante — tu ne restes pas bloqué.

Si on te demande des changements : corrige, repousse, réponds au commentaire. Pas besoin de rouvrir une PR.

Une PR sans activité pendant **3 jours** reçoit un rappel automatique : à ce stade elle a pris du retard sur
`main`, il faut la resynchroniser, la faire relancer, ou la fermer si elle est abandonnée.

---

## 7. Le cas particulier : toucher un contrat partagé

```
packages/shared/src/protocol.ts   types.ts   schema.ts   constants.ts   map.ts
packages/server/src/systems/index.ts
```

Ces fichiers sont lus par tout le monde : un changement y bloque tous les autres. C'est le **seul** point où
on se sérialise.

1. **Avant d'écrire le code**, ouvre une issue qui décrit l'ajout, label `contrat`, et fais-la valider par un
   mainteneur.
2. Le changement part dans une PR **`contract: …` qui ne contient que ça** — relue et mergée seule, en
   premier. La CI refuse une PR de contrat qui embarque du code, et une PR normale qui touche un contrat.
3. **Uniquement des ajouts** : un champ optionnel, un nouvel événement. On ne renomme et on ne supprime jamais
   le champ d'un autre.
4. Puis tout le monde resynchronise : `git merge origin/main`.

---

## 8. Quand ça coince

| Symptôme | Ce qui se passe |
|---|---|
| CI `areas` rouge : « Area inconnue » | le scope de ton titre n'est pas dans `.github/areas.json`. Corrige le titre de la PR (pas besoin de repousser). |
| CI `areas` rouge : « hors de l'area » | ta PR touche des fichiers d'un autre domaine. Découpe-la, ou change d'area si tu t'es trompé. |
| CI `areas` rouge : « fichiers de contrat » | sors-les dans une PR `contract:` séparée (§7). |
| « ça marche chez moi » | vérifie `node -v` contre `.nvmrc`. C'est la cause numéro un. |
| Des tests échouent sans que tu aies touché à ça | certains tests dépendent de l'horloge murale et lâchent sur une machine chargée. Relance-les seuls : `npx vitest run packages/server/test/wards.test.ts`. S'ils passent seuls, ce n'est pas toi — voir l'issue de suivi. |
| Conflit dans un fichier que tu ne connais pas | garde la version de `main`, réapplique ton ajout par-dessus. En cas de doute, demande à qui l'a écrit plutôt que de trancher. |
| Tu es bloqué par le travail de quelqu'un | commente **son** issue, et contourne avec un mock en attendant. |

---

## 9. Travailler avec tes propres IA

Chacun amène ses IA. Pour qu'elles ne cassent pas le travail des autres, briefe-les avec ça :

```
Lis AGENTS.md, CONTRIBUTING.md et PLAN.md avant d'écrire une ligne.

Tâche : <colle l'issue>
Area : <l'area de l'issue> — ton périmètre est celui que .github/areas.json donne pour cette area.

Contraintes :
- Tu ne modifies QUE les fichiers de cette area (+ ses tests). Rien d'autre, même si ça t'arrangerait.
- Tu ne touches à aucun fichier de contrat partagé (protocol.ts, types.ts, schema.ts, constants.ts,
  map.ts, systems/index.ts). S'il en faut un, tu t'arrêtes et tu me le dis.
- Tu n'affaiblis, ne skippes et ne supprimes jamais un test existant.
- Avant de proposer la PR : node scripts/check-areas.mjs --title "<titre>", puis
  npm run typecheck && npm test && npm run build, tous verts.
- Textes du jeu en français. Aucun nom, visuel, son ni texte de Riot.
```

Les deux règles qu'une IA enfreint le plus souvent, et qu'il faut lui répéter : **elle sort de son périmètre**
parce que c'était « plus simple de corriger aussi ça », et **elle assouplit un test** qui la gêne. La CI
attrape la première. Pour la seconde, regarde toi-même le diff des tests avant d'ouvrir la PR.

---

## 10. Ce qu'il y a dans `.github/`

| Fichier | Rôle |
|---|---|
| `areas.json` | le découpage du projet. **La source de vérité** : les labels et le contrôle de périmètre en découlent |
| `workflows/ci.yml` | les trois jobs (`check`, `areas`, `size`) et le label d'area |
| `workflows/labels.yml` | recrée les labels `area:*` dès que `areas.json` change. Aucun label n'est supprimé |
| `workflows/verrous.yml` | libère les verrous dormants et réveille les PR qui dorment, une fois par jour |
| `CODEOWNERS` | demande automatiquement la relecture d'un mainteneur, et surveille les fichiers de contrat |
| `pull_request_template.md` | le corps de PR à remplir |
| `ISSUE_TEMPLATE/` | `tache.md`, `bug.md`, `champion.md` |

Les scripts correspondants (`scripts/check-areas.mjs`, `sync-labels.mjs`, `stale-locks.mjs`) tournent aussi en
local : tous acceptent `--dry-run` ou `--title/--files` pour que tu puisses vérifier avant de pousser.

---

## 11. Ce qui n'existe plus

Si une doc, un script ou une IA te parle de ça, c'est périmé — le projet a changé de modèle quand il s'est
ouvert à plusieurs contributeurs :

- `YOUR_PLAN.md` (un fichier de mission différent par branche) ;
- une branche longue par personne (`claude-fx`, `mistral-ui`…) ;
- `scripts/open-pr.sh`, `scripts/safe-merge.sh`, les branches `pr/<agent>` ;
- `docs/plans/` et `docs/progress/`, déplacés dans [`docs/archive/`](docs/archive/) : à lire comme mémoire du
  design, pas comme une attribution de tâches.
