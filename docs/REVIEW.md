# REVIEW.md — rôle du mainteneur (relecture et intégration)

Les mainteneurs sont les **seuls** à merger sur `main`. Ils n'écrivent pas de fonctionnalités : ils relisent,
demandent des changements, mergent, et gardent la cohérence du jeu.

Avant ta première revue, lis `AGENTS.md`, `PLAN.md` et `.github/areas.json`.

## Ce que la CI vérifie à ta place

Ne refais pas à la main ce qui est déjà mécanique. La CI bloque déjà :

| Job | Garantit |
|---|---|
| `check` | typecheck, tests, build client, 8 bots WebSocket en charge |
| `areas` | la PR reste dans l'area de son titre ; aucun contrat touché hors PR `contract:` ; une PR `champion` ne couvre qu'un champion |
| `size` | avertit au-delà de ~800 lignes |
| `rust` | `cargo fmt`, `clippy -D warnings`, `cargo test` sur `crates/`, 8 bots contre le serveur Rust (~3 min, le plus lent) |

**CI rouge → changements demandés, on ne merge pas.** Point final, pas de discussion à avoir.

La protection de `main` n'exige que `check` et `areas`. `rust` n'est pas bloquant pour GitHub, mais il l'est
pour toi : attends-le vert, sauf si tu viens de faire passer fmt, clippy et les tests Rust en local sur le commit
exact que tu merges.

## Ce que toi seul peux juger

1. **Cohérence de design** : est-ce que ça va dans le sens de `PLAN.md` ? Est-ce que ça double un système existant ?
2. **Contrats** : un ajout dans `protocol.ts` / `types.ts` / `schema.ts` / `constants.ts` / `map.ts` /
   `systems/index.ts` est-il vraiment nécessaire, et bien nommé ? C'est le seul changement irréversible en
   pratique : tout le monde va construire dessus. Une PR `contract:` se merge **seule et en premier**.
3. **Tests non affaiblis** : les tests des autres areas passent toujours, sans avoir été assouplis,
   `skip`és ou supprimés pour faire passer la PR. `git diff origin/main...HEAD -- '**/test/**'` le montre vite.
4. **Qualité** (`AGENTS.md §5`) : pas de `any` non justifié, pas de `@ts-ignore`, pas d'allocation par tick
   ou par frame dans les boucles chaudes, pas de crate non justifiée. Côté Rust : pas d'`unwrap()`
   hors tests sur une donnée venue du réseau, pas de `#[allow(...)]` sans commentaire, et `lov-sim` ne lit
   ni l'horloge ni un aléatoire non semé (`Instant::now`, `SystemTime`, `thread_rng` dans un diff de
   `lov-sim` = changement demandé).
5. **Propriété intellectuelle** (`AGENTS.md §6`) : aucun nom, visuel, son ni texte de Riot ; assets générés
   ou CC0 listés dans `ASSETS.md` ; textes du jeu en français.
6. **Issue liée** : la PR référence une issue (`Closes #n`, ou `Refs #n` si elle n'en livre qu'une partie,
   comme un kit de champion sans modèle ni FX), et l'issue était bien assignée à son auteur —
   sinon deux personnes ont pu travailler sur les mêmes fichiers.

## Boucle de travail

```bash
gh pr list --base main --state open        # la plus ancienne d'abord
gh pr view <n>                             # template rempli ? issue liée ?
gh pr checks <n>                           # tout vert ?
gh pr diff <n>
```

Pour tester localement sans toucher à ton checkout, garde **un seul** worktree de relecture et réutilise-le
d'une PR à l'autre : son dossier `target/` reste chaud. Un worktree neuf recompile tout depuis zéro.
```bash
git worktree add --detach ../lov-review origin/main      # une fois
cd ../lov-review && git fetch origin && git checkout --detach origin/main
git merge --no-edit origin/<branche-de-la-pr>            # teste la PR telle qu'elle sera sur main
cargo fmt --all --check
cargo clippy -p lov-sim -p lov-shared --all-targets -- -D warnings
cargo test -p lov-sim -p lov-shared                      # ~1 min ; ajoute -p lov-server si la PR y touche
```
Évite `cargo test --workspace` en local pour une PR qui ne touche pas `lov-client` : il recompile tout Bevy
(un quart d'heure à froid) pour rien. La CI, elle, teste tout le workspace.


## Décision

- **Changements demandés** : liste précise, `fichier:ligne` + ce qu'il faut faire. L'auteur corrige sur sa
  branche et repousse : la PR se met à jour toute seule.
  ```bash
  gh pr review <n> --request-changes --body-file /tmp/review-<n>.md
  ```
- **Accepté** :
  ```bash
  gh pr review <n> --approve --body "CI verte, area respectée, contrats inchangés."
  gh pr merge <n> --squash --delete-branch
  ```
  `--squash` : une tâche = un commit sur `main`. L'historique de `main` reste lisible et chaque entrée
  correspond à une issue. (L'ancien modèle imposait `--merge` parce que les branches longues se
  resynchronisaient ; il n'y a plus de branches longues.)
- **PR non mergeable** (conflit) : demande à l'auteur de faire `git merge origin/main` sur sa branche.
  Ne résous pas ses conflits à sa place.
- **Toute PR que tu ne merges pas reçoit un commentaire** qui dit pourquoi et ce que l'auteur doit faire
  ensuite (CI rouge, conflit, contrat à sortir…). Ton résumé dans ton terminal, l'auteur ne le voit pas :
  sans commentaire, il ne sait pas qu'il est bloqué.
- On ne peut pas approuver sa propre PR : un mainteneur qui merge la sienne le fait sans `--approve`
  (aucune approbation n'est exigée par la protection), après la même relecture.

## Cas fréquents

- **Série de PR de champions** : chacune ajoute deux lignes voisines au registre
  (`data/champions/mod.rs`, ou un `index.ts`). Dès que l'une est mergée, toutes les autres passent en
  conflit. Merge-les une par une : resynchronise la suivante, attends sa CI, merge, puis la suivante.
  Les resynchroniser toutes d'avance gaspille la CI, puisque chaque merge les recasse. La résolution prend la
  version de `main` et réinsère les deux lignes : `pub mod` par ordre alphabétique, l'entrée de la table
  **dans l'ordre du TypeScript** : `CHAMPION_LIST` pour les données (`data/champions/mod.rs`, vérifié par
  `tests/champions.rs`), `CHAMPION_ABILITIES` de `abilities/registry.ts` pour les kits
  (`abilities/champions/mod.rs`, non testé : les passifs s'installent dans cet ordre).
- **PR empilées** (B contient A, C contient B…) : merge dans l'ordre de la pile. Après le squash de A, B
  passe souvent en conflit sur les mêmes registres (`mod.rs`, `game_systems()`), même si les fichiers ajoutés
  sont identiques. Une pile qui embarque un fichier de contrat garde son job `areas` rouge tant que le contrat
  n'est pas sorti dans sa propre PR `contract:` : c'est normal, ne la merge pas en l'état.
- **CI verte, mais sur un ancien `main`** : la protection n'exige pas une branche à jour. Si tu viens de
  merger une PR qui touche les mêmes registres (`systems/mod.rs`, `tests/common/`), teste localement
  `main` + la PR suivante avant de la merger. Dans une série (kits de champions…), fais-le pour **chaque**
  PR, même sans conflit : un kit peut casser le test d'un autre kit mergé entre-temps.
- **Un test qui échoue, ce n'est pas forcément la faute de la PR** : avant de demander des changements,
  relance-le plusieurs fois sur `main` seul. Un test qui passe avec `--test-threads=1` mais échoue en
  parallèle trahit un état global partagé entre tests (registre `static`, `register_*` sans portée par
  monde, ids d'entités identiques d'un `World` à l'autre) : ouvre une issue pour l'auteur du test, et ne
  bloque pas la PR.

## Garder la file courte

C'est le vrai travail d'un mainteneur. Une PR qui attend trois jours prend du retard sur `main`, conflicte,
et son auteur enchaîne sur une branche qui dérive.

- Relis par ordre d'arrivée, plusieurs fois par jour.
- Une PR trop grosse pour être relue : renvoie-la découpée **tout de suite**, sans la relire en détail.
- Deux mainteneurs valent mieux qu'un : personne ne doit attendre qu'une seule personne soit disponible.
- Ne pousse pas de commit sur la branche d'autrui. Si une correction triviale bloque, demande-la.
