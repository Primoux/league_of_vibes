//! Connexion au serveur (protocole JSON v2) et état de partie reçu.
//!
//! Les messages sont les types de `lov_shared::protocol` (`ClientMsg` / `ServerMsg`), les entités
//! celles de `lov_shared::types`. PROTOTYPE : l'interpolation et la prédiction ne sont pas portées.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use bevy::prelude::*;
use lov_shared::protocol::{
    ClientMsg, DraftState, PROTOCOL_VERSION, RoomPhase, ServerMsg, Snapshot, apply_entity_patch,
    decode, encode,
};
use lov_shared::types::{EntityId, EntitySnapshot};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

const DEFAULT_URL: &str = "ws://localhost:3000/ws";
/// Champions tentés au draft, dans l'ordre, quand le serveur ne tourne pas avec `SKIP_DRAFT=1`.
const DRAFT_ORDER: [&str; 6] = ["grondak", "kaelen", "ilyra", "sable", "nyx", "solenne"];

/// Canal vers le fil réseau, en messages typés.
#[derive(Resource)]
pub struct Connection {
    outgoing: Sender<ClientMsg>,
    incoming: Mutex<Receiver<ServerMsg>>,
}

impl Connection {
    pub fn send(&self, msg: ClientMsg) {
        // Le fil réseau s'arrête quand la connexion tombe : un envoi perdu est alors sans conséquence.
        let _ = self.outgoing.send(msg);
    }
}

/// Dernier état connu de la partie, reconstruit à partir des snapshots complets et delta.
#[derive(Resource, Default)]
pub struct GameState {
    pub status: String,
    pub player_id: Option<String>,
    pub my_entity: Option<EntityId>,
    pub entities: HashMap<EntityId, EntitySnapshot>,
    pub snapshots: u64,
    start_requested: bool,
}

impl GameState {
    pub fn me(&self) -> Option<&EntitySnapshot> {
        self.entities.get(&self.my_entity?)
    }
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

/// Mode hors ligne (scène de charge) : mêmes ressources, mais aucun fil réseau. Les ordres envoyés
/// sont perdus car le récepteur du canal est abandonné.
pub struct OfflinePlugin;

impl Plugin for OfflinePlugin {
    fn build(&self, app: &mut App) {
        let (outgoing, _) = mpsc::channel();
        let (_, incoming) = mpsc::channel();
        app.insert_resource(Connection {
            outgoing,
            incoming: Mutex::new(incoming),
        })
        .init_resource::<GameState>();
    }
}

/// Signale un problème de connexion au jeu sous la forme d'un `ServerMsg` d'erreur.
fn report(to_game: &Sender<ServerMsg>, message: String) {
    let _ = to_game.send(ServerMsg::Error { message });
}

fn network_thread(url: &str, from_game: &Receiver<ClientMsg>, to_game: &Sender<ServerMsg>) {
    let mut socket = match tungstenite::connect(url) {
        Ok((socket, _)) => socket,
        Err(e) => return report(to_game, format!("Connexion à {url} impossible : {e}")),
    };
    // Lecture à délai court : la même boucle sert les envois et les réceptions.
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(2)));
    }
    let hello = ClientMsg::Hello {
        name: "Natif".into(),
        version: PROTOCOL_VERSION,
        reconnect_token: None,
    };
    if !send_msg(&mut socket, &hello) {
        return report(to_game, "Connexion fermée pendant le hello".into());
    }
    loop {
        loop {
            match from_game.try_recv() {
                Ok(msg) => {
                    if !send_msg(&mut socket, &msg) {
                        return report(to_game, "Connexion perdue".into());
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        match socket.read() {
            // Donnée venue du réseau : un message illisible est journalisé puis ignoré.
            Ok(Message::Text(raw)) => match decode::<ServerMsg>(raw.as_str()) {
                Ok(msg) => {
                    if to_game.send(msg).is_err() {
                        return;
                    }
                }
                Err(e) => warn!("message serveur illisible, ignoré : {e}"),
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

/// Encode et envoie un message ; `false` si la connexion est tombée.
fn send_msg(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>, msg: &ClientMsg) -> bool {
    match encode(msg) {
        Ok(raw) => socket.send(Message::text(raw)).is_ok(),
        Err(e) => {
            warn!("message client non encodable, ignoré : {e}");
            true
        }
    }
}

fn pump_network(conn: Res<Connection>, mut state: ResMut<GameState>) {
    // Le verrou n'est jamais disputé : ce système est le seul lecteur.
    let Ok(incoming) = conn.incoming.lock() else {
        return;
    };
    while let Ok(msg) = incoming.try_recv() {
        match msg {
            ServerMsg::Welcome { player_id, .. } => {
                state.player_id = Some(player_id);
                state.status = "Création de la salle…".into();
                conn.send(ClientMsg::CreateRoom {
                    dev_mode: Some(true),
                });
            }
            ServerMsg::Lobby { state: lobby } => {
                if lobby.phase == RoomPhase::Lobby && !state.start_requested {
                    state.start_requested = true;
                    state.status = "Salle créée, lancement…".into();
                    conn.send(ClientMsg::SetReady { ready: true });
                    conn.send(ClientMsg::StartGame);
                }
            }
            ServerMsg::Draft { state: draft } => {
                state.status = "Draft…".into();
                lock_first_free_champion(&conn, &state, &draft);
            }
            ServerMsg::Loading { .. } => state.status = "Chargement…".into(),
            ServerMsg::GameStart { start } => {
                state.my_entity = Some(start.entity_id);
                state.status = "En jeu".into();
                info!("partie lancée, entité du joueur : {:?}", state.my_entity);
            }
            ServerMsg::Snapshot { snap } => apply_snapshot(&mut state, *snap),
            ServerMsg::GameOver { .. } => state.status = "Partie terminée".into(),
            ServerMsg::Error { message } => {
                warn!("serveur : {message}");
                state.status = message;
            }
            ServerMsg::Pong { .. } => {}
        }
    }
}

fn lock_first_free_champion(conn: &Connection, state: &GameState, draft: &DraftState) {
    let Some(me) = state.player_id.as_deref() else {
        return;
    };
    let my_turn = draft
        .turns
        .get(draft.current_turn as usize)
        .is_some_and(|turn| turn.iter().any(|p| p == me));
    let locked = draft.picks.iter().any(|p| p.player_id == me && p.locked);
    if !my_turn || locked {
        return;
    }
    let free = DRAFT_ORDER.iter().find(|id| {
        !draft
            .picks
            .iter()
            .any(|p| p.champion_id.as_deref() == Some(**id))
    });
    if let Some(id) = free {
        conn.send(ClientMsg::DraftLock {
            champion_id: (*id).to_string(),
        });
    }
}

/// Même sémantique que le client web : sans `removed`, le snapshot est complet ; avec, c'est un delta
/// dont `patches` porte les mises à jour partielles (voir `apply_entity_patch` de `lov-shared`).
fn apply_snapshot(state: &mut GameState, snap: Snapshot) {
    state.snapshots += 1;
    if state.snapshots == 1 {
        info!("premier snapshot reçu");
    }
    if !snap.is_delta() {
        state.entities.clear();
    }
    for entity in snap.entities {
        state.entities.insert(entity.id, entity);
    }
    for id in snap.removed.iter().flatten() {
        state.entities.remove(id);
    }
    for patch in snap.patches.iter().flatten() {
        if let Some(entity) = state.entities.get_mut(&patch.id) {
            *entity = apply_entity_patch(entity, patch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::protocol::{EntityField, EntityPatch, Score, TeamScore};
    use lov_shared::types::{EntityKind, TeamOrNeutral};

    fn ent(id: EntityId, x: f64) -> EntitySnapshot {
        EntitySnapshot {
            id,
            kind: EntityKind::Minion,
            team: TeamOrNeutral::Blue,
            x,
            y: 5.0,
            facing: 0.0,
            hp: 1.0,
            max_hp: 1.0,
            mana: None,
            max_mana: None,
            shield: None,
            dead: None,
            subtype: None,
            owner_id: None,
            level: None,
            anim: Some("run".into()),
            buffs: None,
            champ: None,
        }
    }

    fn snap(
        entities: Vec<EntitySnapshot>,
        removed: Option<Vec<EntityId>>,
        patches: Option<Vec<EntityPatch>>,
    ) -> Snapshot {
        let team = || TeamScore {
            kills: 0,
            towers: 0,
            dragons: vec![],
            grubs: 0,
        };
        Snapshot {
            tick: 0,
            time: 0.0,
            server_time: 0.0,
            entities,
            removed,
            patches,
            events: vec![],
            self_state: None,
            objective_timers: None,
            score: Score {
                blue: team(),
                red: team(),
            },
        }
    }

    fn state_after(snaps: Vec<Snapshot>) -> GameState {
        let mut state = GameState::default();
        for s in snaps {
            apply_snapshot(&mut state, s);
        }
        state
    }

    #[test]
    fn snapshot_complet_remplace_tout() {
        let state = state_after(vec![
            snap(vec![ent(1, 10.0), ent(2, 20.0)], None, None),
            snap(vec![ent(2, 25.0)], None, None),
        ]);
        assert_eq!(state.entities.len(), 1);
        assert_eq!(state.entities[&2].x, 25.0);
    }

    #[test]
    fn delta_conserve_les_absentes_et_retire_les_removed() {
        let state = state_after(vec![
            snap(vec![ent(1, 10.0), ent(2, 20.0), ent(3, 30.0)], None, None),
            snap(vec![ent(4, 40.0)], Some(vec![2]), None),
        ]);
        let mut ids: Vec<_> = state.entities.keys().copied().collect();
        ids.sort_unstable();
        assert_eq!(ids, [1, 3, 4]);
    }

    #[test]
    fn patch_modifie_et_supprime_des_champs() {
        let mut patch = EntityPatch::new(1);
        patch.x = Some(12.0);
        patch.u = Some(vec![EntityField::Anim]);
        let state = state_after(vec![
            snap(vec![ent(1, 10.0)], None, None),
            snap(vec![], Some(vec![]), Some(vec![patch])),
        ]);
        let e = &state.entities[&1];
        assert_eq!(e.x, 12.0);
        assert_eq!(e.y, 5.0);
        assert_eq!(e.anim, None);
    }

    #[test]
    fn patch_d_une_entite_inconnue_est_ignore() {
        let mut patch = EntityPatch::new(9);
        patch.x = Some(1.0);
        let state = state_after(vec![snap(vec![], Some(vec![]), Some(vec![patch]))]);
        assert!(state.entities.is_empty());
    }
}
