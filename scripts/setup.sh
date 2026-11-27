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

# .agent dit aux IA de cette machine qui elles sont. Non versionné.
echo "$who" > .agent
echo "Identité de ce clone : $who (.agent)"

# On ne touche PAS à l'identité git : la tienne est déjà la bonne, et GitHub attribue les
# commits sur l'adresse e-mail. On prévient seulement si elle manque.
if [ -z "$(git config user.email)" ]; then
  echo "⚠ git config user.email n'est pas réglé : tes commits ne seront rattachés à aucun compte GitHub."
  echo "  Règle-le avec l'adresse de ton compte : git config --global user.email \"toi@exemple.com\""
fi

# Le jeu est en Rust. rustup lit rust-toolchain.toml et installe la bonne version.
if ! command -v cargo >/dev/null; then
  echo "⚠ Rust n'est pas installé : installe rustup (https://rustup.rs), puis relance ce script." >&2
  exit 1
fi
if [ "$(uname)" = Linux ]; then
  echo "Linux : Bevy a besoin de libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev (paquets apt)."
fi
cargo test --workspace

# Node ne sert plus qu'aux scripts du dépôt (garde-fou des areas, labels).
if command -v node >/dev/null; then
  node --test scripts/check-areas.test.ts
fi
echo
echo "Prêt. Prends une issue libre : gh issue list --label 'bonne première tâche'"
