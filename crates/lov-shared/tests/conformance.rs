//! Conformité sur trafic RÉEL : chaque ligne des fixtures a été enregistrée depuis le serveur Node
//! (`server.jsonl` = serveur -> client, `client.jsonl` = client -> serveur ; voir
//! `fixtures/README`-style note dans le message de commit). Chaque message doit se décoder dans
//! les types Rust puis se ré-encoder en un JSON équivalent : un champ présent d'un côté seulement
//! est un échec (c'est le bug que ce test doit attraper).

mod common;

use common::{lines, round_trip};
use lov_shared::protocol::{ClientMsg, ServerMsg, apply_entity_patch};
use lov_shared::types::EntitySnapshot;
use serde_json::Value;
use std::collections::BTreeSet;

fn str_field(line: &str, key: &str) -> String {
    let v: Value = serde_json::from_str(line).expect("json");
    v[key].as_str().unwrap_or("?").to_owned()
}

#[test]
fn serveur_vers_client() {
    let lines = lines("server.jsonl");
    assert!(lines.len() > 30, "fixture trop petite");
    let mut types = BTreeSet::new();
    let mut events = BTreeSet::new();
    let (mut fulls, mut deltas, mut patches, mut unsets, mut removed) = (0, 0, 0, 0, 0);
    for line in &lines {
        let msg: ServerMsg = round_trip(line);
        types.insert(str_field(line, "t"));
        if let ServerMsg::Snapshot { snap } = &msg {
            if snap.is_delta() {
                deltas += 1;
            } else {
                fulls += 1;
            }
            removed += snap.removed.as_ref().map_or(0, Vec::len);
            patches += snap.patches.as_ref().map_or(0, Vec::len);
            unsets += snap
                .patches
                .iter()
                .flatten()
                .filter(|p| p.u.is_some())
                .count();
            for e in &snap.events {
                events.insert(str_field(&serde_json::to_string(e).expect("evt"), "type"));
            }
        }
    }
    eprintln!("types serveur: {types:?}\névénements: {events:?}");
    eprintln!(
        "complets={fulls} deltas={deltas} patches={patches} avec-u={unsets} removed={removed}"
    );
    for t in ["welcome", "lobby", "error", "gameStart", "snapshot", "pong"] {
        assert!(types.contains(t), "type serveur {t} absent des fixtures");
    }
    for e in [
        "damage",
        "autoAttack",
        "death",
        "abilityCast",
        "buffApplied",
        "projectileSpawn",
    ] {
        assert!(events.contains(e), "événement {e} absent des fixtures");
    }
    assert!(fulls >= 1 && deltas >= 1 && patches >= 1 && unsets >= 1 && removed >= 1);
}

#[test]
fn client_vers_serveur() {
    let lines = lines("client.jsonl");
    assert!(lines.len() > 20, "fixture trop petite");
    let mut types = BTreeSet::new();
    for line in &lines {
        let _: ClientMsg = round_trip(line);
        types.insert(str_field(line, "t"));
    }
    eprintln!("types client: {types:?}");
    for t in [
        "hello",
        "createRoom",
        "move",
        "attackMove",
        "attack",
        "cast",
        "levelAbility",
        "buy",
    ] {
        assert!(types.contains(t), "type client {t} absent des fixtures");
    }
}

/// Rejoue un flux de snapshots : complet puis deltas (patches, removed, entités nouvelles). Les
/// fixtures sont échantillonnées, donc on ignore les patches dont l'entité est inconnue (le client
/// réel ne les reçoit jamais).
#[test]
fn les_patches_se_rejouent() {
    let mut state: Vec<EntitySnapshot> = Vec::new();
    let mut patched = 0;
    for line in lines("server.jsonl") {
        let ServerMsg::Snapshot { snap } = round_trip::<ServerMsg>(&line) else {
            continue;
        };
        match &snap.removed {
            None => state.clone_from(&snap.entities),
            Some(removed) => {
                state.retain(|e| !removed.contains(&e.id));
                for p in snap.patches.iter().flatten() {
                    if let Some(i) = state.iter().position(|e| e.id == p.id) {
                        state[i] = apply_entity_patch(&state[i], p);
                        patched += 1;
                    }
                }
                for e in &snap.entities {
                    match state.iter().position(|x| x.id == e.id) {
                        Some(i) => state[i] = e.clone(),
                        None => state.push(e.clone()),
                    }
                }
            }
        }
    }
    assert!(!state.is_empty());
    assert!(patched > 0, "aucun patch rejoué");
}
