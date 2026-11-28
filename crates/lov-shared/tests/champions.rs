//! Champions (`data/champions/`) comparés champ par champ à l'instantané `fixtures/champions.json`.
//!
//! `fixtures/champions.json` est un objet `id -> ChampionDef` dans l'ordre du registre. Il a d'abord
//! été généré en important `packages/shared/src/data/champions/index.ts` avec Node (valeurs finales,
//! après les aides `stats()` et `ability()` de `_helpers.ts`) : le portage Rust lui est identique.
//! Depuis la bascule, c'est lui la référence : un champion ajouté ou rééquilibré se livre avec
//! l'instantané régénéré par `scripts/bless-fixtures.sh`, dont le diff se relit dans la PR (#276).

mod common;

use common::{check_snapshot, fixture, json_diff};
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
    let Ordered(v) = serde_json::from_str(&fixture("champions.json")).expect("champions.json");
    v
}

/// L'instantané : `id -> ChampionDef` dans l'ordre du registre, une clé par champion pour que le
/// diff d'un rééquilibrage reste lisible (`serde_json::Map` trierait les ids).
#[test]
fn instantane_des_champions() {
    let defs: Vec<(String, Value)> = CHAMPIONS
        .iter()
        .map(|c| {
            (
                c.id.clone(),
                serde_json::to_value(c).expect("sérialisation"),
            )
        })
        .collect();
    let mut text = String::from("{\n");
    for (n, (id, def)) in defs.iter().enumerate() {
        let body = serde_json::to_string_pretty(def).expect("encodage");
        let sep = if n + 1 < defs.len() { "," } else { "" };
        text.push_str(&format!("  {id:?}: {}{sep}\n", body.replace('\n', "\n  ")));
    }
    text.push_str("}\n");
    let as_value = Value::Object(defs.into_iter().collect());
    check_snapshot("champions.json", &text, &as_value);
}

#[test]
fn le_fixture_contient_tous_les_champions_du_registre() {
    let ts = ts();
    assert_eq!(ts.len(), CHAMPIONS.len());
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
