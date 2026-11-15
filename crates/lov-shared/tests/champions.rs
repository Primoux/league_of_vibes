//! Champions (`data/champions/`) comparés champ par champ aux valeurs du TypeScript.
//!
//! `fixtures/ts_champions.json` est un objet `id -> ChampionDef` dans l'ordre de `CHAMPION_LIST`,
//! généré en important `packages/shared/src/data/champions/index.ts` avec Node (valeurs finales,
//! après les aides `stats()` et `ability()` de `_helpers.ts`).
//!
//! Les champions sont portés un par un : le test vérifie tous ceux que le registre Rust contient
//! (dans le même ordre relatif que le TS) et n'a pas à changer quand on en ajoute un.

mod common;

use common::{fixture, json_diff};
use lov_shared::data::champions::{CHAMPIONS, get_champion};
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;

/// Les paires (id, définition) du fixture dans l'ordre du fichier : `serde_json::Map` trie les clés.
struct Ordered(Vec<(String, Value)>);

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("un objet id -> champion")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Ordered, A::Error> {
                let mut v = Vec::new();
                while let Some(kv) = m.next_entry()? {
                    v.push(kv);
                }
                Ok(Ordered(v))
            }
        }
        d.deserialize_map(V)
    }
}

fn ts() -> Vec<(String, Value)> {
    let Ordered(v) =
        serde_json::from_str(&fixture("ts_champions.json")).expect("ts_champions.json");
    v
}

#[test]
fn le_fixture_contient_les_13_champions() {
    let ts = ts();
    assert_eq!(ts.len(), 13);
    for (id, def) in &ts {
        assert_eq!(def["id"], *id);
    }
}

#[test]
fn champions_identiques_au_ts() {
    let ts = ts();
    assert!(!CHAMPIONS.is_empty());
    // Ordre : le registre Rust est une sous-suite de l'ordre TS.
    let mut next = 0;
    for c in CHAMPIONS.iter() {
        let pos = ts[next..]
            .iter()
            .position(|(id, _)| *id == c.id)
            .unwrap_or_else(|| panic!("{} : inconnu du TS ou hors de l'ordre du TS", c.id));
        let (id, expected) = &ts[next + pos];
        next += pos + 1;
        let actual = serde_json::to_value(c).expect("sérialisation");
        if let Some(d) = json_diff(expected, &actual, id) {
            panic!("{d}");
        }
    }
    let ids: HashSet<&str> = CHAMPIONS.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), CHAMPIONS.len(), "id en double");
}

#[test]
fn recherche_par_id() {
    let g = get_champion("grondak").expect("grondak");
    assert_eq!(g.name, "Grondak");
    assert!(std::ptr::eq(
        g,
        CHAMPIONS
            .iter()
            .copied()
            .find(|c| c.id == "grondak")
            .unwrap()
    ));
    for c in CHAMPIONS.iter() {
        assert_eq!(
            get_champion(&c.id).map(|x| x.id.as_str()),
            Some(c.id.as_str())
        );
    }
    assert!(get_champion("inconnu").is_none());
    assert!(get_champion("").is_none());
    assert!(get_champion("Grondak").is_none());
}
