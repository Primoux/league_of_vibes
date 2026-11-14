//! Messages ABSENTS du trafic enregistré (le serveur lancé avec `SKIP_DRAFT=1` n'envoie ni `draft`
//! ni `loading`, et une partie courte ne produit ni `gameOver` ni événements de structures ou
//! d'objectifs). Ces JSON sont écrits À LA MAIN d'après `protocol.ts` / `types.ts` : ils vérifient
//! la forme attendue, pas le comportement réel du serveur.

mod common;

use common::round_trip;
use lov_shared::protocol::{ClientMsg, ServerMsg};
use lov_shared::types::GameEvent;

#[test]
fn messages_serveur_non_enregistres() {
    let cases = [
        r#"{"t":"loading","endsAt":1700000000000}"#,
        r#"{"t":"draft","state":{"turns":[["p1"],["p2","p3"]],"currentTurn":1,"turnEndsAt":1700000030000,"picks":[{"playerId":"p1","team":"blue","championId":"grondak","hoverChampionId":null,"locked":true,"summoners":["flash","ignite"]},{"playerId":"p2","team":"red","championId":null,"hoverChampionId":"x","locked":false,"summoners":["heal","ghost"]}],"finalizeEndsAt":1700000060000}}"#,
        r#"{"t":"draft","state":{"turns":[],"currentTurn":0,"turnEndsAt":1,"picks":[]}}"#,
        r#"{"t":"gameOver","result":{"winner":"red","duration":1234.5,"stats":[{"playerId":"p1","championId":"grondak","team":"blue","kills":1,"deaths":2,"assists":3,"cs":40,"gold":5000,"damageToChampions":12345.5,"items":["doran_blade",null]}]}}"#,
        r#"{"t":"pong","serverTime":5,"clientTime":4}"#,
        r#"{"t":"lobby","state":{"code":"ABCDE","phase":"draft","players":[{"id":"p1","name":"A","team":"blue","ready":true,"isHost":true,"connected":true,"role":"jungle"}],"devMode":false}}"#,
    ];
    for c in cases {
        let _: ServerMsg = round_trip(c);
    }
}

#[test]
fn evenements_non_enregistres() {
    let cases = [
        r#"{"type":"areaEffect","source":1,"fxKey":"k","pos":{"x":1,"y":2},"radius":300,"duration":2}"#,
        r#"{"type":"itemSold","target":1,"itemId":"doran_blade"}"#,
        r#"{"type":"itemUsed","target":1,"itemId":"health_potion"}"#,
        r#"{"type":"structureDestroyed","target":9,"kind":"inhibitor","team":"red","plate":true}"#,
        r#"{"type":"structureDestroyed","target":9,"kind":"nexus","team":"blue"}"#,
        r#"{"type":"objectiveSpawn","monster":"dragon","element":"hextech","pos":{"x":7900,"y":4100}}"#,
        r#"{"type":"objectiveSpawn","monster":"grub","pos":{"x":4100,"y":7900}}"#,
        r#"{"type":"objectiveTaken","monster":"elder_dragon","team":"blue","killer":3,"element":"cloud"}"#,
        r#"{"type":"announcement","key":"dragon_taken","team":"red","params":{"n":2,"who":"x"}}"#,
        r#"{"type":"announcement","key":"nexus_down"}"#,
        r#"{"type":"projectileHit","id":4,"pos":{"x":0,"y":0},"fxKey":"k"}"#,
        r#"{"type":"respawn","target":1,"pos":{"x":500,"y":500}}"#,
        r#"{"type":"death","target":1,"killer":2,"assists":[3,4],"bounty":300}"#,
        r#"{"type":"goldGain","target":1,"amount":20,"pos":{"x":1,"y":1}}"#,
        r#"{"type":"levelUp","target":1,"level":2,"fxKey":"lvl"}"#,
        r#"{"type":"summonerCast","source":1,"spell":"flash","from":{"x":1,"y":1},"to":{"x":2,"y":2},"targetId":5,"fxKey":"f"}"#,
        r#"{"type":"recallEnd","target":1,"completed":false}"#,
    ];
    for c in cases {
        let _: GameEvent = round_trip(c);
    }
}

#[test]
fn messages_client_non_enregistres() {
    let cases = [
        r#"{"t":"hello","name":"A","version":2,"reconnectToken":"tok"}"#,
        r#"{"t":"createRoom","devMode":true}"#,
        r#"{"t":"leaveRoom"}"#,
        r#"{"t":"returnToLobby"}"#,
        r#"{"t":"cast","slot":"R","targetPos":{"x":1,"y":2},"targetId":3}"#,
        r#"{"t":"cast","slot":"Q"}"#,
        r#"{"t":"summoner","slot":"F","targetId":3}"#,
        r#"{"t":"useItem","slot":-1,"targetPos":{"x":1,"y":2}}"#,
        r#"{"t":"draftSummoners","summoners":["smite","cleanse"]}"#,
        r#"{"t":"netPing","clientTime":1700000000123.5}"#,
    ];
    for c in cases {
        let _: ClientMsg = round_trip(c);
    }
}
