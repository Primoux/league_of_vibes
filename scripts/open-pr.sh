#!/usr/bin/env bash
# Ouvre (ou met à jour) la PR de ta branche d'agent vers main.
# Usage : scripts/open-pr.sh ["titre de la PR"] [corps.md]   (depuis ta branche d'agent)
#   corps.md : description remplie à partir de .github/pull_request_template.md
#              (par défaut : le template brut, à compléter avec `gh pr edit --body-file`).
#
# Une PR directe <agent> → main apporterait ton YOUR_PLAN.md sur main. Ce script prépare
# donc une branche pr/<agent> = origin/main + ta branche, avec le YOUR_PLAN.md de main
# conservé (via safe-merge.sh), la pousse et ouvre la PR avec le template.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

branch="$(git rev-parse --abbrev-ref HEAD)"
case "$branch" in
  main|pr/*) echo "Lance ce script depuis ta branche d'agent (pas depuis $branch)." >&2; exit 1 ;;
esac
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "Arbre de travail non propre : commit ou stash d'abord." >&2
  exit 1
fi
command -v gh >/dev/null || { echo "gh (GitHub CLI) est requis." >&2; exit 1; }

title="${1:-$(git log -1 --format=%s)}"
body="${2:-.github/pull_request_template.md}"
[ -f "$body" ] || { echo "Fichier de description introuvable : $body" >&2; exit 1; }
body="$(realpath "$body")"
pr_branch="pr/$branch"

git fetch origin
git push origin "$branch"

git checkout -q -B "$pr_branch" origin/main
trap 'git checkout -q "$branch"' EXIT
scripts/safe-merge.sh "$branch"

npm run typecheck
npm test

git push --force-with-lease origin "$pr_branch"

if gh pr view "$pr_branch" --json state -q .state 2>/dev/null | grep -q OPEN; then
  [ -n "${2:-}" ] && gh pr edit "$pr_branch" --body-file "$body"
  echo "PR existante mise à jour :"
  gh pr view "$pr_branch" --json url -q .url
else
  gh pr create --base main --head "$pr_branch" --title "$title" --body-file "$body"
fi
