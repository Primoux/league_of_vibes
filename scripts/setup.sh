#!/usr/bin/env bash
# Prépare ce clone pour un contributeur (une machine = un clone = une identité).
#   usage : scripts/setup.sh <ton-pseudo>      ex. scripts/setup.sh enzo
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

who="${1:-}"
if [ -z "$who" ]; then
  echo "usage: scripts/setup.sh <ton-pseudo>" >&2
  exit 1
fi

echo "$who" > .agent                       # non versionné : dit aux IA de cette machine qui elles sont
git config user.name "$who"                # sinon toutes les contributions sont indistinguables
echo "Identité de ce clone : $who  (.agent + git config user.name)"

if command -v node >/dev/null; then
  want="$(cat .nvmrc)"; have="$(node -v | sed 's/^v//;s/\..*//')"
  [ "$have" = "$want" ] || echo "⚠ Node $have installé, $want attendu (.nvmrc) — la CI tourne en $want."
fi

npm install
npm run typecheck
npm test
echo
echo "Prêt. Prends une issue libre : gh issue list --label 'bonne première tâche'"
