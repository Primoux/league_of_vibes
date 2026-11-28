#!/usr/bin/env bash
# Régénère les instantanés de données depuis le Rust (#276) : à lancer quand une donnée, un champion ou
# la carte change VOLONTAIREMENT, puis relire le diff des fixtures avant de pousser (AGENTS.md §5).
#
#   crates/lov-shared/tests/fixtures/{champions,data,tables}.json
#   crates/lov-sim/tests/fixtures/nav_paths.json
set -euo pipefail
cd "$(dirname "$0")/.."

LOV_BLESS=1 cargo test -q -p lov-shared --test champions --test data --test map -- instantane tables_identiques
LOV_BLESS=1 cargo test -q -p lov-sim --test nav -- memes_chemins_que_l_instantane
# Relecture : les instantanés régénérés passent sans LOV_BLESS.
cargo test -q -p lov-shared --test champions --test data --test map
cargo test -q -p lov-sim --test nav

git --no-pager diff --stat -- crates/lov-shared/tests/fixtures crates/lov-sim/tests/fixtures
