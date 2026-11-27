//! Tables de données (`data/**`) comparées champ par champ à l'instantané `fixtures/data.json`.
//!
//! L'instantané a d'abord été généré en important `packages/shared/src/data/*.ts` avec Node (chaque
//! `Record` dumpé en liste de paires `[clé, valeur]` dans l'ordre d'insertion, `ITEM_LIST` en liste) :
//! le portage Rust lui est identique. Depuis la bascule, c'est lui la référence, régénéré depuis le
//! Rust par `scripts/bless-fixtures.sh` quand une donnée change volontairement (#276).
//! L'ordre des clés d'objet est ignoré par `json_diff`, pas celui des listes : l'ordre des entrées
//! est donc vérifié.

mod common;

use common::{check_snapshot, fixture, json_diff, pretty};
use lov_shared::data::buffs::{BUFFS, buff};
use lov_shared::data::items::{ITEM_LIST, item};
use lov_shared::data::minions::{MINIONS, minion};
use lov_shared::data::monsters::{DRAGON_ELEMENTS, MONSTERS, monster};
use lov_shared::data::structures::STRUCTURES;
use lov_shared::data::summoners::{DEFAULT_SUMMONERS, SUMMONER_IDS, SUMMONERS, summoner};
use lov_shared::types::{MinionType, MonsterType, SummonerId, TowerTier};
use serde_json::Value;
use std::collections::HashSet;

fn ts() -> Value {
    serde_json::from_str(&fixture("data.json")).expect("data.json")
}

/// L'instantané complet, dans la forme de `fixtures/data.json`.
fn snapshot() -> Value {
    let pairs = |v: Vec<(Value, Value)>| {
        Value::Array(
            v.into_iter()
                .map(|(k, d)| Value::Array(vec![k, d]))
                .collect(),
        )
    };
    serde_json::json!({
        "buffs": pairs(BUFFS.iter().map(|b| (j(&b.id), j(b))).collect()),
        "items": j(&*ITEM_LIST),
        "itemsRecordKeys": j(&ITEM_LIST.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()),
        "minions": j(&MINIONS),
        "monsters": j(&MONSTERS),
        "dragonElements": j(&DRAGON_ELEMENTS),
        "structures": j(&STRUCTURES),
        "summoners": pairs(SUMMONERS.iter().map(|s| (j(&s.id), j(s))).collect()),
        "summonerIds": j(&SUMMONER_IDS),
        "defaultSummoners": j(&DEFAULT_SUMMONERS),
    })
}

/// Toutes les tables à la fois ; `LOV_BLESS=1` réécrit l'instantané (voir `common::check_snapshot`).
#[test]
fn instantane_des_donnees() {
    let v = snapshot();
    check_snapshot("data.json", &pretty(&v), &v);
}

fn j<T: serde::Serialize + ?Sized>(v: &T) -> Value {
    serde_json::to_value(v).expect("sérialisation")
}

fn same(name: &str, rust: &Value, ts: &Value) {
    if let Some(d) = json_diff(ts, rust, name) {
        panic!("{d}");
    }
}

/// `Object.entries(record)` : les clés, dans l'ordre.
fn keys(v: &Value) -> Vec<&str> {
    v.as_array()
        .expect("liste de paires")
        .iter()
        .map(|p| p[0].as_str().expect("clé"))
        .collect()
}

#[test]
fn buffs_identiques_au_ts() {
    let ts = ts();
    same(
        "buffs",
        &j(&*BUFFS),
        &Value::Array(
            ts["buffs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p[1].clone())
                .collect(),
        ),
    );
    // `BUFFS` est un Record indexé par id : la clé est l'id.
    for p in ts["buffs"].as_array().unwrap() {
        let id = p[0].as_str().unwrap();
        assert_eq!(buff(id).map(|b| b.id.as_str()), Some(id));
    }
    assert!(buff("inconnu").is_none());
}

#[test]
fn objets_identiques_au_ts() {
    let ts = ts();
    same("items", &j(&*ITEM_LIST), &ts["items"]);
    // `ITEMS` = `Object.fromEntries(ITEM_LIST.map(i => [i.id, i]))` : mêmes clés, dans l'ordre.
    let ids: Vec<&str> = ITEM_LIST.iter().map(|i| i.id.as_str()).collect();
    let ts_keys: Vec<&str> = ts["itemsRecordKeys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect();
    assert_eq!(ids, ts_keys);
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    for (n, i) in ITEM_LIST.iter().enumerate() {
        assert!(item(&i.id).is_some_and(|x| std::ptr::eq(x, &ITEM_LIST[n])));
        // Tous les composants existent.
        for c in &i.builds_from {
            assert!(item(c).is_some(), "{} : composant {c} inconnu", i.id);
        }
    }
    assert!(item("inconnu").is_none());
}

#[test]
fn sbires_identiques_au_ts() {
    let ts = ts();
    same("minions", &j(&MINIONS), &ts["minions"]);
    for (kind, def) in MINIONS {
        assert_eq!(minion(kind), &def);
    }
    let order: Vec<MinionType> = MINIONS.iter().map(|(k, _)| *k).collect();
    assert_eq!(keys(&ts["minions"]), ["melee", "caster", "cannon", "super"]);
    assert_eq!(
        order,
        [
            MinionType::Melee,
            MinionType::Caster,
            MinionType::Cannon,
            MinionType::Super
        ]
    );
}

#[test]
fn monstres_identiques_au_ts() {
    let ts = ts();
    same("monsters", &j(&MONSTERS), &ts["monsters"]);
    for (kind, def) in MONSTERS {
        assert_eq!(monster(kind), &def);
    }
    // Les clés sérialisées sont les chaînes du protocole.
    for ((kind, _), k) in MONSTERS.iter().zip(keys(&ts["monsters"])) {
        assert_eq!(kind.as_str(), k);
    }
    assert_eq!(monster(MonsterType::Dragon).team_gold, Some(25.0));
    assert_eq!(monster(MonsterType::Gromp).team_gold, None);
    same(
        "dragonElements",
        &j(&DRAGON_ELEMENTS),
        &ts["dragonElements"],
    );
}

#[test]
fn structures_identiques_au_ts() {
    let ts = ts();
    same("structures", &j(&STRUCTURES), &ts["structures"]);
    assert_eq!(STRUCTURES.towers.get(TowerTier::Outer).hp, 3400.0);
    assert_eq!(STRUCTURES.towers.get(TowerTier::Nexus).local_gold, 50.0);
}

#[test]
fn sorts_d_invocateur_identiques_au_ts() {
    let ts = ts();
    let pairs: Vec<Value> = SUMMONERS
        .iter()
        .map(|s| Value::Array(vec![j(&s.id), j(s)]))
        .collect();
    same("summoners", &Value::Array(pairs), &ts["summoners"]);
    same("summonerIds", &j(&SUMMONER_IDS), &ts["summonerIds"]);
    same(
        "defaultSummoners",
        &j(&DEFAULT_SUMMONERS),
        &ts["defaultSummoners"],
    );
    for id in SUMMONER_IDS {
        assert_eq!(summoner(id).id, id);
    }
    assert_eq!(DEFAULT_SUMMONERS, [SummonerId::Flash, SummonerId::Ignite]);
    // `SUMMONER_IDS` = `Object.keys(SUMMONERS)`.
    let ids: Vec<SummonerId> = SUMMONERS.iter().map(|s| s.id).collect();
    assert_eq!(ids, SUMMONER_IDS);
}
