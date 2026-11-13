# AGENTS.md — règles communes aux 7 IA

Tu es l'une des 7 IA qui construisent **League of Vibes**, un MOBA 4v4 navigateur aux mécaniques de League of Legends.

## 0. Avant d'écrire une ligne
1. Lis **`PLAN.md`** en entier : design complet du jeu, architecture, contrats.
2. Lis **`YOUR_PLAN.md`** : ta mission, tes dossiers, tes jalons. Il est propre à ta branche.
3. Lis `docs/progress/*.md` : où en sont les autres. `docs/plans/` contient les plans de toutes les branches.
4. `npm install && npm run typecheck && npm test` doivent passer avant que tu commences.

## 1. Équipe et branches

| Agent | Branche | Domaine |
|---|---|---|
| claude-server (lead intégration) | `claude-server` | Serveur, réseau, salles, draft, pathfinding, combat, vision |
| claude-client | `claude-client` | Moteur 3D client, caméra, carte, input, interpolation |
| claude-fx | `claude-fx` | FX, shaders, modèles et animations des champions |
| codex-champions | `codex-champions` | Moteur de sorts, buffs/CC, XP, 10 kits, sorts d'invocateur |
| codex-objectives | `codex-objectives` | Sbires, tours, inhibiteurs, nexus, jungle, drakes, larves, colosse, Léviathan |
| mistral-ui | `mistral-ui` | Tous les écrans et le HUD (Preact) |
| mistral-economy | `mistral-economy` | Or, objets, boutique, rappel, balises, audio |

**Fable** (relecteur, sans branche) relit et merge toutes les PR vers `main` — voir `docs/REVIEW.md`.

Tu travailles **uniquement sur ta branche**. Tu ne pousses jamais sur la branche d'un autre agent.

## 2. Propriété des fichiers
- Tu ne modifies **que les fichiers/dossiers listés dans ton `YOUR_PLAN.md`** (+ ton `docs/progress/<agent>.md`, + tes tests).
- Fichiers partagés sensibles :
  - `packages/shared/src/protocol.ts`, `types.ts` → claude-server.
  - `packages/shared/src/schema.ts` → codex-champions (partie items : mistral-economy).
  - `packages/shared/src/map.ts` → claude-client. `constants.ts` → claude-server (bloc `TIMERS` : codex-objectives).
  - `packages/server/src/systems/index.ts` → claude-server ; chacun peut y **ajouter une ligne** pour enregistrer un nouveau système.
- Besoin de changer un contrat qui ne t'appartient pas ? Fais-le **en ajout** (nouveau champ optionnel, nouvel événement), dans un **commit séparé préfixé `contract:`**, et note-le dans ton progress (section « Contrats modifiés »). Ne renomme ni ne supprime jamais un champ existant d'un autre. claude-server arbitre en cas de conflit.
- Communication entre domaines serveur : via l'`EventBus` (`world.events.on/emit`), `entity.data.<ton-domaine>`, et les API documentées dans `PLAN.md §2.1` (`applyDamage`, `addBuff`…). **N'édite pas le système d'un autre** pour y brancher ta logique.
- Communication côté client : `store` (`client/src/state.ts`) et `fxBus` (`client/src/fx/FxBus.ts`).
- Une API dont tu dépends n'existe pas encore ? Mocke-la localement dans tes fichiers, ou crée-la avec la signature exacte du PLAN dans le fichier prévu avec une implémentation minimale, et signale-le.

## 3. Git
- Configuration à faire une fois : `chmod +x scripts/safe-merge.sh scripts/open-pr.sh`, et `gh auth login`.
- **Synchronise-toi souvent avec `main`** (au moins à chaque début de session) :
  ```bash
  git fetch origin
  scripts/safe-merge.sh origin/main   # merge en conservant TON YOUR_PLAN.md
  ```
  N'utilise pas `git merge`/`git pull` bruts depuis main : ils peuvent écraser ton `YOUR_PLAN.md`.
- **Tout passe par une Pull Request vers `main`.** Ne pousse **jamais** directement sur `main` (ni merge local + push). À chaque jalon (et dès qu'un morceau utile aux autres est stable) :
  ```bash
  npm run typecheck && npm test && npm run build      # obligatoire, vert
  cp .github/pull_request_template.md /tmp/pr-body.md # remplis-le (résumé, contrats, tests…)
  scripts/open-pr.sh "feat(<domaine>): ..." /tmp/pr-body.md
  ```
  Le script crée `pr/<ta-branche>` = `origin/main` + ta branche **avec le `YOUR_PLAN.md` de main conservé**, relance typecheck/tests, la pousse et ouvre la PR (ou met à jour la PR déjà ouverte). N'ouvre pas de PR directe `<ta-branche>` → `main` : la CI la refuse car elle remplacerait `YOUR_PLAN.md`.
  - La PR doit avoir la CI verte et suivre le template (`.github/pull_request_template.md`) : checklist cochée honnêtement, section « Contrats modifiés » remplie.
  - Si `main` a bougé ou si la CI échoue : corrige sur ta branche, puis relance `scripts/open-pr.sh`.
  - **Seul Fable (agent relecteur) accepte et merge les PR** (voir `docs/REVIEW.md`). Ne merge jamais ta propre PR, ni celle d'un autre. Tu continues à travailler sur ta branche en attendant.
  - Si Fable demande des changements (commentaire « Changements demandés » sur la PR) : corrige sur ta branche, relance `scripts/open-pr.sh`, et réponds au commentaire.
- Conflits : résous-les en préservant le travail des deux côtés. Dans un fichier qui ne t'appartient pas, garde la version de son propriétaire et réapplique ton ajout.
- Commits conventionnels en anglais ou français : `feat(server): ...`, `fix(fx): ...`, `contract: ...`, `test: ...`, `docs: ...`. Petits commits fréquents. Push régulier sur ta branche (`git push origin <ta-branche>`).

## 4. Qualité
- TypeScript strict, pas de `any` sans commentaire justifiant. Pas de `// @ts-ignore`.
- `npm run typecheck && npm test` **verts avant chaque push**. Le client doit aussi builder (`npm run build`).
- Logique de jeu testée en headless avec vitest (`packages/server/test/`) : crée un `World`, ajoute des entités, fais des `step()`, vérifie. Chaque agent serveur ajoute des tests pour ses règles.
- Côté client, ajoute une page de démo quand c'est utile (ex. `?fxlab`, `?uilab`) pour tester sans serveur.
- Performances : pas d'allocation inutile dans les boucles par tick/par frame, réutilise les objets.
- Pas de nouvelle dépendance npm sans la justifier dans ton progress. Pas de framework concurrent (pas de React, pas de Babylon…).

## 5. Contenu et propriété intellectuelle
- Mécaniques de League of Legends : oui. **Noms, champions, visuels, sons, textes de Riot : jamais.** Utilise les noms du PLAN (Léviathan, Colosse du Rift, Larves, Faille des Vibes…).
- Assets : générés (code, shaders, zzfx) ou **CC0** uniquement. Tout asset ajouté est listé dans `ASSETS.md` (fichier, source, licence).
- Textes du jeu en français.

## 6. Progression
- Tiens `docs/progress/<agent>.md` à jour à chaque session : fait / en cours / bloqué / contrats modifiés / ce dont tu as besoin des autres.
- Si tu es bloqué par un autre agent, écris-le dans ton progress (section « Besoins ») et contourne avec un mock en attendant.
- Ne t'arrête pas à la première tâche : enchaîne les jalons de `YOUR_PLAN.md` jusqu'à M4.
