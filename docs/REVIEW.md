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

**CI rouge → changements demandés, on ne merge pas.** Point final, pas de discussion à avoir.

## Ce que toi seul peux juger

1. **Cohérence de design** : est-ce que ça va dans le sens de `PLAN.md` ? Est-ce que ça double un système existant ?
2. **Contrats** : un ajout dans `protocol.ts` / `types.ts` / `schema.ts` / `constants.ts` / `map.ts` /
   `systems/index.ts` est-il vraiment nécessaire, et bien nommé ? C'est le seul changement irréversible en
   pratique : tout le monde va construire dessus. Une PR `contract:` se merge **seule et en premier**.
3. **Tests non affaiblis** : les tests des autres areas passent toujours, sans avoir été assouplis,
   `skip`és ou supprimés pour faire passer la PR. `git diff origin/main...HEAD -- '**/test/**'` le montre vite.
4. **Qualité** (`AGENTS.md §5`) : pas de `any` non justifié, pas de `@ts-ignore`, pas d'allocation par tick
   ou par frame dans les boucles chaudes, pas de dépendance npm non justifiée.
5. **Propriété intellectuelle** (`AGENTS.md §6`) : aucun nom, visuel, son ni texte de Riot ; assets générés
   ou CC0 listés dans `ASSETS.md` ; textes du jeu en français.
6. **Issue liée** : la PR référence une issue (`Closes #n`), et l'issue était bien assignée à son auteur —
   sinon deux personnes ont pu travailler sur les mêmes fichiers.

## Boucle de travail

```bash
gh pr list --base main --state open        # la plus ancienne d'abord
gh pr view <n>                             # template rempli ? issue liée ?
gh pr checks <n>                           # tout vert ?
gh pr diff <n>
```

Pour tester localement sans toucher à ton checkout :
```bash
git fetch origin && git worktree add /tmp/review-<n> origin/<branche-de-la-pr>
cd /tmp/review-<n> && npm ci && npm run typecheck && npm test && npm run build
git worktree remove /tmp/review-<n>        # après
```

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

## Garder la file courte

C'est le vrai travail d'un mainteneur. Une PR qui attend trois jours prend du retard sur `main`, conflicte,
et son auteur enchaîne sur une branche qui dérive.

- Relis par ordre d'arrivée, plusieurs fois par jour.
- Une PR trop grosse pour être relue : renvoie-la découpée **tout de suite**, sans la relire en détail.
- Deux mainteneurs valent mieux qu'un : personne ne doit attendre qu'une seule personne soit disponible.
- Ne pousse pas de commit sur la branche d'autrui. Si une correction triviale bloque, demande-la.
