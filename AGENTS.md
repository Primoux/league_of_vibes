# AGENTS.md — règles de contribution (humains et IA)

Tu contribues à **League of Vibes**, un MOBA 4v4 navigateur aux mécaniques de League of Legends.
Plusieurs personnes y travaillent, chacune sur sa machine, chacune avec ses propres IA. Ces règles existent
pour qu'on puisse être nombreux **sans se marcher dessus** et sans faire exploser la file de relecture.

## 0. Avant d'écrire une ligne

1. Lis **`PLAN.md`** : le design du jeu, l'architecture, les contrats.
2. Prépare ta machine, une seule fois : `scripts/setup.sh <ton-pseudo>`.
   (Écrit `.agent` — non versionné, c'est l'identité de ce clone — règle `git config user.name`, installe, teste.)
3. `npm install && npm run typecheck && npm test` doivent passer **avant** que tu commences.
4. Prends une tâche : voir §1.

Il n'y a **pas** de fichier `YOUR_PLAN.md`, pas de branche réservée par personne, pas de dossier possédé à vie.
Si une doc ou une IA t'en parle, elle est périmée : le modèle a changé (voir `docs/archive/`).

## 1. Une issue = une tâche = une PR

Le tableau de bord, c'est les **Issues GitHub** : c'est le seul endroit visible depuis toutes les machines.

```bash
gh issue list --state open                     # ce qui est libre
gh issue list --assignee "" --label area:fx-core
gh issue edit <n> --add-assignee @me           # ← PRENDRE la tâche : ça vaut verrou
```

**S'assigner une issue, c'est poser un verrou sur ses fichiers.** Tant qu'une issue est assignée à
quelqu'un d'autre, tu ne touches pas à son périmètre. C'est ce qui remplace la propriété de dossiers :
ça marche entre machines, et deux personnes ne peuvent pas prendre la même issue.

- Rien ne te va ? Ouvre une issue (`.github/ISSUE_TEMPLATE/tache.md`) et assigne-la toi.
- Tu abandonnes ? Désassigne-toi tout de suite, pour libérer le verrou.
- Une tâche qui déborde de son area, c'est **deux** issues.

## 2. Areas : le découpage du projet

`.github/areas.json` est la source de vérité : chaque **area** liste les chemins qu'elle couvre.
`node scripts/check-areas.mjs --title "feat(ui-hud): x"` te dit si ta PR tient dans son area ; la CI le refait.

**Le scope de ton titre de PR est l'area** : `feat(fx-core): …`, `fix(ui-hud): …`, `feat(nav): …`.

### Tranches verticales : un champion = une tâche

Le code stocke **un fichier par champion** à chaque étage, ce qui permet de livrer un champion de bout en
bout sans croiser personne :

```
packages/shared/src/data/champions/<nom>.ts          stats, sorts, coûts
packages/server/src/abilities/champions/<nom>.ts     logique serveur
packages/client/src/render/champions/models/<nom>.ts  modèle 3D
packages/client/src/fx/champions/<nom>.ts            FX et sons
```

C'est le découpage à privilégier : vingt contributeurs peuvent livrer vingt champions en parallèle sans un
seul conflit. Une PR `champion` ne couvre **qu'un** champion (la CI le vérifie) ; seuls les `index.ts` de
registre sont partagés, en ajout d'une ligne.

Même logique pour les objets, les monstres de jungle, les écrans d'UI : préfère une tranche complète et
étanche à un bout de chantier qui traverse trois areas.

## 3. Contrats partagés : le seul point de sérialisation

```
packages/shared/src/protocol.ts   types.ts   schema.ts   constants.ts   map.ts
packages/server/src/systems/index.ts
```

Ces fichiers sont lus par tout le monde : un changement y bloque tous les autres. Donc :

1. **Avant d'écrire le code**, ouvre une issue qui décrit l'ajout et fais-la valider par un mainteneur.
2. Le changement part dans une **PR `contract:` qui ne contient que ça** — elle est relue et mergée seule,
   en premier. La CI refuse une PR de contrat qui embarque autre chose, et une PR normale qui touche un contrat.
3. **Uniquement des ajouts** : un nouveau champ optionnel, un nouvel événement. On ne renomme ni ne supprime
   jamais un champ existant sans l'accord d'un mainteneur.
4. Puis chacun resynchronise sa branche sur `main`.

Une API dont tu dépends n'existe pas encore ? Mocke-la chez toi, ou crée-la avec la signature exacte du
`PLAN.md` et une implémentation minimale, et dis-le dans ta PR.

**Communication entre domaines**, plutôt que d'éditer le code d'un autre :
- serveur : `EventBus` (`world.events.on/emit`), `entity.data.<domaine>`, les API de `PLAN.md §2.1` (`applyDamage`, `addBuff`…) ;
- client : le `store` (`client/src/state.ts`) et le `fxBus` (`client/src/fx/FxBus.ts`).

## 4. Git

```bash
git fetch origin && git switch -c <area>/<slug> origin/main   # toujours repartir d'un main à jour
# … travaille, commits petits et fréquents …
npm run typecheck && npm test && npm run build                # obligatoire, vert
git push -u origin <area>/<slug>
gh pr create --fill                                           # remplis le template
```

- **Branches courtes de vie** : une tâche, une branche, mergée dans la journée, supprimée.
  Pas de branche longue par personne : on a déjà vu des branches finir à 231 commits de retard sur `main`,
  impossibles à merger.
- **Tout passe par une PR vers `main`.** Ne pousse jamais sur `main` (la protection le refuse de toute façon).
- `main` a bougé ? `git fetch origin && git merge origin/main` dans ta branche, puis repousse.
- **Tu ne merges pas ta propre PR** : un mainteneur relit, valide et merge (voir `docs/REVIEW.md`).
- Conflit dans un fichier que tu ne connais pas : garde la version de `main` et réapplique ton ajout par-dessus.
- Commits conventionnels, en français ou en anglais : `feat(ui-hud): …`, `fix(nav): …`, `contract: …`, `test: …`, `docs: …`.
- **PR petites** : au-delà de ~800 lignes la CI prévient, parce que la relecture devient approximative.

## 5. Qualité

- TypeScript strict. Pas de `any` sans commentaire qui le justifie. Pas de `// @ts-ignore`.
- `npm run typecheck && npm test` verts avant chaque push ; le client doit builder (`npm run build`).
- Logique de jeu testée en headless avec vitest (`packages/*/test/`) : crée un `World`, ajoute des entités,
  fais des `step()`, vérifie. Toute règle de jeu que tu ajoutes vient avec son test.
- **On n'affaiblit ni ne supprime jamais le test d'un autre** pour faire passer sa PR. Si un test d'autrui
  casse à cause de toi, c'est ton code ou le contrat qui doit changer — dis-le dans la PR.
- Côté client, ajoute une page de démo quand c'est utile (`?fxlab`, `?uilab`…) pour tester sans serveur.
- Performances : pas d'allocation inutile par tick ou par frame, réutilise les objets.
- Pas de nouvelle dépendance npm sans la justifier dans la PR. Pas de framework concurrent (pas de React, pas de Babylon…).

## 6. Contenu et propriété intellectuelle

- Mécaniques de League of Legends : oui. **Noms, champions, visuels, sons, textes de Riot : jamais.**
  Utilise les noms du `PLAN.md` (Léviathan, Colosse du Rift, Larves, Faille des Vibes…).
- Assets : générés (code, shaders, zzfx) ou **CC0** uniquement, et listés dans `ASSETS.md` (fichier, source, licence).
- Textes du jeu en français.

## 7. Où en est le projet

- L'avancement se lit dans les **issues et les PR**, pas dans un fichier du dépôt : un fichier de suivi
  partagé conflicte à chaque merge.
- Bloqué par le travail de quelqu'un d'autre ? Commente **son** issue, et contourne avec un mock en attendant.
- `docs/archive/` garde les plans et les journaux de la première phase du projet (sept IA, une branche
  chacune) : utile comme mémoire du design, périmé comme mode d'emploi.
