#!/usr/bin/env bash
# Merge <ref> dans la branche courante en CONSERVANT le YOUR_PLAN.md de la branche courante.
# Usage : scripts/safe-merge.sh origin/main      (depuis ta branche)
#         scripts/safe-merge.sh claude-server    (depuis main)
set -euo pipefail
ref="${1:?usage: scripts/safe-merge.sh <ref>}"
cd "$(git rev-parse --show-toplevel)"

if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "Arbre de travail non propre : commit ou stash d'abord." >&2
  exit 1
fi

keep="$(mktemp)"
trap 'rm -f "$keep"' EXIT
had_plan=0
if git cat-file -e HEAD:YOUR_PLAN.md 2>/dev/null; then
  git show HEAD:YOUR_PLAN.md > "$keep"
  had_plan=1
fi

set +e
git merge --no-ff --no-commit "$ref"
merge_status=$?
set -e

if ! git rev-parse -q --verify MERGE_HEAD >/dev/null; then
  echo "Rien à merger (déjà à jour)."
  exit 0
fi

# Restaurer le YOUR_PLAN.md de la branche courante, quoi que contienne <ref>.
if [ "$had_plan" = 1 ]; then
  cp "$keep" YOUR_PLAN.md
  git add YOUR_PLAN.md
else
  git rm -q --cached --ignore-unmatch YOUR_PLAN.md
  rm -f YOUR_PLAN.md
fi

if [ "$merge_status" -ne 0 ] || [ -n "$(git diff --name-only --diff-filter=U)" ]; then
  echo
  echo "Conflits à résoudre (YOUR_PLAN.md déjà restauré) :"
  git diff --name-only --diff-filter=U
  echo "Résous-les, 'git add' les fichiers, puis 'git commit --no-edit'."
  exit 1
fi

git commit --no-edit -q
echo "Merge de $ref terminé (YOUR_PLAN.md conservé)."
