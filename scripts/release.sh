#!/usr/bin/env bash
# Publier un patch de League of Vibes (mainteneur). Voir docs/patch-notes/README.md.
#
#   scripts/release.sh 0.2.0         assemble docs/patch-notes/next/ dans patchs.md, monte la version du
#                                    workspace, pousse la branche release/v0.2.0 et ouvre sa PR
#   scripts/release.sh --tag 0.2.0   une fois la PR mergée : pose l'étiquette v0.2.0 sur origin/main ;
#                                    build-natif.yml construit les archives et publie la release
set -euo pipefail
cd "$(dirname "$0")/.."

tag=false
if [ "${1:-}" = "--tag" ]; then tag=true; shift; fi
version="${1:-}"
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage : scripts/release.sh [--tag] <version x.y.z>" >&2
  exit 2
fi

git fetch -q origin
cargo_version() { git show "$1:Cargo.toml" | sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p'; }

if $tag; then
  current=$(cargo_version origin/main)
  if [ "$current" != "$version" ]; then
    echo "origin/main est en version $current, pas $version : la PR de release est-elle mergée ?" >&2
    exit 1
  fi
  git show origin/main:docs/patch-notes/patchs.md | grep -q "^# Patch ${version%.0} " \
    || { echo "Aucune note pour ce patch dans patchs.md (origin/main)." >&2; exit 1; }
  git tag -a "v$version" origin/main -m "League of Vibes $version"
  git push origin "v$version"
  echo "✓ v$version poussée : la release se construit (onglet Actions, build-natif)."
  exit 0
fi

[ -z "$(git status --porcelain)" ] || { echo "Le dépôt a des modifications non commitées." >&2; exit 1; }
git switch -c "release/v$version" origin/main
node scripts/patch-notes.mjs check
node scripts/patch-notes.mjs assemble "$version"
sed -i.bak '/^\[workspace.package\]/,/^\[/s/^version = ".*"/version = "'"$version"'"/' Cargo.toml && rm Cargo.toml.bak
cargo update --workspace --offline -q 2>/dev/null || cargo update --workspace -q
git add -A docs/patch-notes Cargo.toml Cargo.lock
git commit -q -m "chore(infra): patch ${version%.0}"
git push -q -u origin "release/v$version"
gh pr create --title "chore(infra): patch ${version%.0}" --body "Publication du patch ${version%.0} : notes assemblées dans \`docs/patch-notes/patchs.md\`, version du workspace à $version.

Après le merge : \`scripts/release.sh --tag $version\`.

$(node scripts/patch-notes.mjs extract "$version")"
