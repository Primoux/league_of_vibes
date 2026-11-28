# Notes de patch

Le jeu a une version, `0.N`, comme un patch : elle s'affiche sur l'accueil du client, et le bouton
« Notes de patch » liste ce qui a changé à chaque version. Ce texte vient de [`patchs.md`](patchs.md),
embarqué dans le client à la compilation.

## Quand ajouter une note

Une PR qui change quelque chose **que le joueur peut remarquer** ajoute sa note : un chiffre
d'équilibrage, un sort, un objet, un camp de jungle, une règle de jeu, un écran, un bug corrigé qui se
voyait. Une refactorisation, un test, la CI ou la doc n'en ont pas besoin.

## Comment : un fragment par PR

Chaque PR crée **son propre fichier** dans [`next/`](next/), nommé d'après sa branche
(`next/items-sort-lame.md`). Personne n'édite un fichier partagé, donc personne n'entre en conflit.
`docs/**/*.md` est autorisé dans toutes les areas.

```markdown
## Champions
### Corvin
- Q : dégâts 60/95/130 ⇒ 70/105/140
- W : délai de récupération 14 s ⇒ 12 s

## Objets
### Grand glaive
- Le prix tient compte des deux épées longues possédées.

## Correctifs
- Une tour ne se bloque plus sur un champion invisible.
```

- Catégories, dans cet ordre : `Champions`, `Objets`, `Jungle et objectifs`, `Système`, `Interface`,
  `Correctifs`. Une autre est refusée par la CI.
- `### Sujet` est facultatif. Pour un champion ou un objet, écris le **nom affiché en jeu** : le
  client montre son portrait ou son icône à côté.
- Une ligne par changement, en français, du point de vue du joueur. Pour un chiffre : `avant ⇒ après`.
  Une ligne qui commence par `+` s'affiche en vert (amélioration), par `−` ou `-` en rouge
  (affaiblissement).

`node scripts/patch-notes.mjs check` valide les fragments (la CI le fait aussi).

## Publier un patch (mainteneur)

```bash
scripts/release.sh 0.2.0          # assemble next/ dans patchs.md, monte la version, ouvre la PR
# … la PR est relue et mergée : la CI prépare toute seule un brouillon de release …
scripts/release.sh --tag 0.2.0    # publie le brouillon (admin), ce qui pose l'étiquette v0.2.0
```

Chaque patch a donc toujours ses notes : la CI ne prépare la release que si `patchs.md` contient la
section du patch, et le brouillon reprend ce texte, suivi de la liste des PR. Le bouton « Publish » du
brouillon, sur GitHub, revient au même que `--tag`.
