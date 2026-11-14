//! Connexion au serveur (protocole JSON v2) et état de partie reçu.
//!
//! PROTOTYPE : `lov-shared` n'existe pas encore, donc les messages sont manipulés comme du JSON brut
//! (`serde_json::Value`) au lieu des types du protocole. À remplacer par les types de `lov-shared`
//! dès que la PR `contract:` est mergée. L'interpolation et la prédiction ne sont pas portées.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use bevy::prelude::*;
use serde_json::{Map, Value, json};
use tungstenite::Message;
use tungstenite::stream::MaybeTlsStream;

/// Doit rester égal à `PROTOCOL_VERSION` de `packages/shared/src/protocol.ts`.
const PROTOCOL_VERSION: u32 = 2;
const DEFAULT_URL: &str = "ws://localhost:3000/ws";
/// Champions tentés au draft, dans l'ordre, quand le serveur ne tourne pas avec `SKIP_DRAFT=1`.
const DRAFT_ORDER: [&str; 6] = ["grondak", "kaelen", "ilyra", "sable", "nyx", "solenne"];

pub type EntityData = Map<String, Value>;

/// Canal vers le fil réseau. Les messages sont du JSON conforme à `ClientMsg` / `ServerMsg`.
#[derive(Resource)]
pub struct Connection {
    outgoing: Sender<Value>,
    incoming: Mutex<Receiver<Value>>,
}

impl Connection {
    pub fn send(&self, msg: Value) {
        // Le fil réseau s'arrête quand la connexion tombe : un envoi perdu est alors sans conséquence.
        let _ = self.outgoing.send(msg);
    }
}

/// Dernier état connu de la partie, reconstruit à partir des snapshots complets et delta.
#[derive(Resource, Default)]
pub struct GameState {
    pub status: String,
    pub player_id: Option<String>,
    pub my_entity: Option<u64>,
    pub entities: HashMap<u64, EntityData>,
    pub snapshots: u64,
    start_requested: bool,
}

impl GameState {
    pub fn me(&self) -> Option<&EntityData> {
        self.entities.get(&self.my_entity?)
    }
}

/// Lit un nombre d'une entité (`x`, `y`, `hp`…), 0 si absent.
pub fn num(e: &EntityData, key: &str) -> f32 {
    e.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32
}

pub fn text<'a>(e: &'a EntityData, key: &str) -> &'a str {
    e.get(key).and_then(Value::as_str).unwrap_or("")
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        let url = std::env::var("LOV_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
        let (outgoing, from_game) = mpsc::channel();
        let (to_game, incoming) = mpsc::channel();
        thread::spawn(move || network_thread(&url, &from_game, &to_game));
        app.insert_resource(Connection {
            outgoing,
            incoming: Mutex::new(incoming),
        })
        .insert_resource(GameState {
            status: "Connexion…".into(),
            ..default()
        })
        .add_systems(PreUpdate, pump_network);
    }
}

/// Signale un problème de connexion au jeu sous la forme d'un `ServerMsg` d'erreur.
fn report(to_game: &Sender<Value>, message: String) {
    let _ = to_game.send(json!({ "t": "error", "message": message }));
}

fn network_thread(url: &str, from_game: &Receiver<Value>, to_game: &Sender<Value>) {
    let mut socket = match tungstenite::connect(url) {
        Ok((socket, _)) => socket,
        Err(e) => return report(to_game, format!("Connexion à {url} impossible : {e}")),
    };
    // Lecture à délai court : la même boucle sert les envois et les réceptions.
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(2)));
    }
    let hello = json!({ "t": "hello", "name": "Natif", "version": PROTOCOL_VERSION });
    if socket.send(Message::text(hello.to_string())).is_err() {
        return report(to_game, "Connexion fermée pendant le hello".into());
    }
    loop {
        loop {
            match from_game.try_recv() {
                Ok(msg) => {
                    if socket.send(Message::text(msg.to_string())).is_err() {
                        return report(to_game, "Connexion perdue".into());
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        match socket.read() {
            Ok(Message::Text(raw)) => match serde_json::from_str::<Value>(raw.as_str()) {
                Ok(msg) => {
                    if to_game.send(msg).is_err() {
                        return;
                    }
                }
                Err(e) => report(to_game, format!("Message illisible : {e}")),
            },
            Ok(Message::Close(_)) => {
                return report(to_game, "Le serveur a fermé la connexion".into());
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return report(to_game, format!("Connexion perdue : {e}")),
        }
    }
}

fn pump_network(conn: Res<Connection>, mut state: ResMut<GameState>) {
    // Le verrou n'est jamais disputé : ce système est le seul lecteur.
    let Ok(incoming) = conn.incoming.lock() else {
        return;
    };
    while let Ok(mut msg) = incoming.try_recv() {
        let kind = msg
            .get("t")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match kind.as_str() {
            "welcome" => {
                state.player_id = msg["playerId"].as_str().map(str::to_string);
                state.status = "Création de la salle…".into();
                conn.send(json!({ "t": "createRoom", "devMode": true }));
            }
            "lobby" => {
                if msg["state"]["phase"] == "lobby" && !state.start_requested {
                    state.start_requested = true;
                    state.status = "Salle créée, lancement…".into();
                    conn.send(json!({ "t": "setReady", "ready": true }));
                    conn.send(json!({ "t": "startGame" }));
                }
            }
            "draft" => {
                state.status = "Draft…".into();
                lock_first_free_champion(&conn, &state, &msg["state"]);
            }
            "loading" => state.status = "Chargement…".into(),
            "gameStart" => {
                state.my_entity = msg["start"]["entityId"].as_u64();
                state.status = "En jeu".into();
                info!("partie lancée, entité du joueur : {:?}", state.my_entity);
            }
            "snapshot" => {
                if let Some(snap) = msg.get_mut("snap") {
                    apply_snapshot(&mut state, snap.take());
                }
            }
            "gameOver" => state.status = "Partie terminée".into(),
            "error" => {
                let message = msg["message"].as_str().unwrap_or("erreur inconnue");
                warn!("serveur : {message}");
                state.status = message.to_string();
            }
            _ => {}
        }
    }
}

fn lock_first_free_champion(conn: &Connection, state: &GameState, draft: &Value) {
    let Some(me) = state.player_id.as_deref() else {
        return;
    };
    let my_turn = draft["turns"][draft["currentTurn"].as_u64().unwrap_or(0) as usize]
        .as_array()
        .is_some_and(|turn| turn.iter().any(|p| p == me));
    let Some(picks) = draft["picks"].as_array() else {
        return;
    };
    let locked = picks
        .iter()
        .any(|p| p["playerId"] == me && p["locked"] == true);
    if !my_turn || locked {
        return;
    }
    let free = DRAFT_ORDER
        .iter()
        .find(|id| !picks.iter().any(|p| p["championId"] == **id));
    if let Some(id) = free {
        conn.send(json!({ "t": "draftLock", "championId": id }));
    }
}

/// Même sémantique que le client web : sans `removed`, le snapshot est complet ; avec, c'est un delta
/// dont `patches` porte les mises à jour partielles (voir `applyEntityPatch` dans `protocol.ts`).
fn apply_snapshot(state: &mut GameState, mut snap: Value) {
    state.snapshots += 1;
    if state.snapshots == 1 {
        info!("premier snapshot reçu");
    }
    let is_delta = snap.get("removed").is_some_and(|r| !r.is_null());
    if !is_delta {
        state.entities.clear();
    }
    if let Some(Value::Array(entities)) = snap.get_mut("entities").map(Value::take) {
        for entity in entities {
            if let Value::Object(data) = entity
                && let Some(id) = data.get("id").and_then(Value::as_u64)
            {
                state.entities.insert(id, data);
            }
        }
    }
    if let Some(removed) = snap["removed"].as_array() {
        for id in removed.iter().filter_map(Value::as_u64) {
            state.entities.remove(&id);
        }
    }
    if let Some(Value::Array(patches)) = snap.get_mut("patches").map(Value::take) {
        for patch in patches {
            let Value::Object(patch) = patch else {
                continue;
            };
            let Some(entity) = patch
                .get("id")
                .and_then(Value::as_u64)
                .and_then(|id| state.entities.get_mut(&id))
            else {
                continue;
            };
            for (key, value) in patch {
                if key == "u" {
                    for unset in value
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                    {
                        entity.remove(unset);
                    }
                } else {
                    entity.insert(key, value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_after(snaps: &[Value]) -> GameState {
        let mut state = GameState::default();
        for snap in snaps {
            apply_snapshot(&mut state, snap.clone());
        }
        state
    }

    #[test]
    fn snapshot_complet_remplace_tout() {
        let state = state_after(&[
            json!({ "entities": [{ "id": 1, "x": 10.0 }, { "id": 2, "x": 20.0 }] }),
            json!({ "entities": [{ "id": 2, "x": 25.0 }] }),
        ]);
        assert_eq!(state.entities.len(), 1);
        assert_eq!(num(&state.entities[&2], "x"), 25.0);
    }

    #[test]
    fn delta_conserve_les_absentes_et_retire_les_removed() {
        let state = state_after(&[
            json!({ "entities": [{ "id": 1, "x": 10.0 }, { "id": 2, "x": 20.0 }, { "id": 3, "x": 30.0 }] }),
            json!({ "entities": [{ "id": 4, "x": 40.0 }], "removed": [2] }),
        ]);
        let mut ids: Vec<_> = state.entities.keys().copied().collect();
        ids.sort_unstable();
        assert_eq!(ids, [1, 3, 4]);
    }

    #[test]
    fn patch_modifie_et_supprime_des_champs() {
        let state = state_after(&[
            json!({ "entities": [{ "id": 1, "x": 10.0, "y": 5.0, "anim": "run" }] }),
            json!({ "entities": [], "removed": [], "patches": [{ "id": 1, "x": 12.0, "u": ["anim"] }] }),
        ]);
        let e = &state.entities[&1];
        assert_eq!(num(e, "x"), 12.0);
        assert_eq!(num(e, "y"), 5.0);
        assert!(!e.contains_key("anim"));
    }
}
