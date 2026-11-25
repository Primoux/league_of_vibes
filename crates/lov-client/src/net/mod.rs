//! Connexion au serveur (protocole JSON v2) et état client : port de `packages/client/src/net/*` et
//! de `state.ts`.
//!
//! - Un fil réseau tient la WebSocket (bloquante, `tungstenite`) : `hello` avec le jeton de session,
//!   reconnexion avec attente croissante, `netPing` toutes les 2 s pour mesurer la latence.
//! - [`GameState`] (le `store` du client web) reçoit lobby, draft, chargement, début et fin de partie,
//!   snapshots fusionnés dans un [`SnapshotBuffer`], état privé du joueur.
//! - Chaque image, [`GameState::poses`] donne la position interpolée de chaque entité au temps
//!   serveur `serveur − INTERP_DELAY_MS`, et la position prédite du champion local.
//! - Les événements de jeu sont publiés en [`ServerEvent`] (le `fxBus` du client web) quand le rendu
//!   interpolé atteint leur snapshot, pour que les FX coïncident avec les positions affichées.
//!
//! La connexion part à la demande (`Connection::connect`, au clic sur « Jouer » des écrans), comme
//! `net.connect(name)` du client web. Sans écrans, ou avec `LOV_AUTOPLAY=1`, un pilote automatique
//! ([`Autoplay`]) se connecte au lancement, crée une salle de dev, la lance et choisit un champion libre.

pub mod clock;
pub mod prediction;
pub mod snapshots;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use lov_shared::constants::INTERP_DELAY_MS;
use lov_shared::data::champions::CHAMPIONS;
use lov_shared::protocol::{
    ClientMsg, DraftState, GameOver, GameStart, LobbyState, PROTOCOL_VERSION, RoomPhase, Score,
    ServerMsg, Snapshot, decode, encode,
};
use lov_shared::types::{EntityId, EntitySnapshot, GameEvent, SelfState};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

pub use clock::ServerClock;
pub use prediction::{MovePredictor, ServerView};
pub use snapshots::{Pose, SnapshotBuffer};

const DEFAULT_URL: &str = "ws://localhost:3000/ws";
/// Attentes successives avant de retenter la connexion (ms).
const RECONNECT_DELAYS: [u64; 5] = [500, 1000, 2000, 4000, 8000];
const PING_EVERY: Duration = Duration::from_secs(2);
/// Au-delà, les événements en attente sont publiés sans attendre (horloge incohérente…).
const MAX_PENDING_SNAPSHOTS: usize = 40;
/// Vitesse prise pour la prédiction tant que l'état privé n'est pas arrivé.
const DEFAULT_MOVE_SPEED: f64 = 335.0;
/// Latence supposée tant qu'aucun `pong` n'est revenu (ms).
const DEFAULT_RTT: f64 = 80.0;

/// Horloge locale (ms depuis l'époque Unix), comme `Date.now()`.
pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// Un événement de jeu, publié quand le rendu interpolé atteint son snapshot (le `fxBus` du web).
#[derive(Message, Debug, Clone)]
pub struct ServerEvent(pub GameEvent);

/// Pilote automatique : connexion au lancement (`LOV_NAME`), salle de dev créée et lancée, premier
/// champion libre verrouillé. Actif tant qu'aucun écran ne le coupe ; `LOV_AUTOPLAY=1` le force.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Autoplay(pub bool);

impl Default for Autoplay {
    fn default() -> Self {
        Self(true)
    }
}

/// Ce qu'il faut au fil réseau pour démarrer, gardé jusqu'à `connect`.
struct Pending {
    url: String,
    from_game: Receiver<ClientMsg>,
    to_game: Sender<ServerMsg>,
}

/// Canal vers le fil réseau, en messages typés.
#[derive(Resource)]
pub struct Connection {
    outgoing: Sender<ClientMsg>,
    incoming: Mutex<Receiver<ServerMsg>>,
    /// Ordres qui comptent pour la prédiction, relus par `pump_network`.
    local_orders: Mutex<Vec<ClientMsg>>,
    /// Fil réseau pas encore lancé (`None` une fois connecté, ou hors ligne).
    pending: Mutex<Option<Pending>>,
}

impl Connection {
    fn new(outgoing: Sender<ClientMsg>, incoming: Receiver<ServerMsg>) -> Self {
        Self {
            outgoing,
            incoming: Mutex::new(incoming),
            local_orders: Mutex::new(Vec::new()),
            pending: Mutex::new(None),
        }
    }

    /// Connexion vers `url`, lancée au premier `connect`.
    fn deferred(url: String) -> Self {
        let (outgoing, from_game) = mpsc::channel();
        let (to_game, incoming) = mpsc::channel();
        let conn = Self::new(outgoing, incoming);
        if let Ok(mut pending) = conn.pending.lock() {
            *pending = Some(Pending {
                url,
                from_game,
                to_game,
            });
        }
        conn
    }

    /// Lance le fil réseau sous le nom `name` (`hello`) ; sans effet s'il tourne déjà. Les messages
    /// envoyés avant restent en file et partent après le `hello`.
    pub fn connect(&self, name: &str) {
        let Some(Pending {
            url,
            from_game,
            to_game,
        }) = self.pending.lock().ok().and_then(|mut p| p.take())
        else {
            return;
        };
        let name = name.to_owned();
        thread::spawn(move || network_thread(&url, &name, &from_game, &to_game));
    }

    /// Le fil réseau a été lancé (ou la connexion est hors ligne).
    #[cfg(test)]
    pub fn started(&self) -> bool {
        self.pending.lock().is_ok_and(|p| p.is_none())
    }

    /// Connexion de test : renvoie aussi le récepteur des messages envoyés.
    #[cfg(test)]
    pub fn for_tests() -> (Self, Receiver<ClientMsg>) {
        let (outgoing, sent) = mpsc::channel();
        let (_, incoming) = mpsc::channel();
        (Self::new(outgoing, incoming), sent)
    }

    pub fn send(&self, msg: ClientMsg) {
        if matches!(
            msg,
            ClientMsg::Move { .. }
                | ClientMsg::AttackMove { .. }
                | ClientMsg::Attack { .. }
                | ClientMsg::Stop
                | ClientMsg::Cast { .. }
        ) && let Ok(mut orders) = self.local_orders.lock()
        {
            orders.push(msg.clone());
        }
        // Le fil réseau s'arrête quand la connexion tombe : un envoi perdu est alors sans conséquence.
        let _ = self.outgoing.send(msg);
    }
}

/// État client (le `store` du client web).
#[derive(Resource, Default)]
pub struct GameState {
    /// Ligne d'état courte pour l'affichage de debug.
    pub status: String,
    pub connected: bool,
    pub player_id: Option<String>,
    pub lobby: Option<LobbyState>,
    pub draft: Option<DraftState>,
    pub game: Option<GameStart>,
    /// Fin du chargement (temps serveur, ms).
    pub loading_ends_at: Option<f64>,
    pub result: Option<GameOver>,
    pub error: Option<String>,
    /// Latence aller-retour lissée (ms).
    pub rtt: Option<f64>,
    /// Entité du joueur local.
    pub my_entity: Option<EntityId>,
    /// Vue complète fusionnée et historique des positions.
    pub buffer: SnapshotBuffer,
    pub clock: ServerClock,
    pub predictor: MovePredictor,
    /// Position affichée de chaque entité, recalculée chaque image.
    pub poses: HashMap<EntityId, Pose>,
    /// Méta-données du dernier snapshot.
    pub tick: u64,
    pub time: f64,
    pub score: Option<Score>,
    pub self_state: Option<SelfState>,
    pub objective_timers: Option<BTreeMap<String, f64>>,
    /// Événements du dernier snapshot (lecture immédiate par l'interface).
    pub events: Vec<GameEvent>,
    /// Snapshots reçus depuis le lancement.
    pub snapshots: u64,
    pending: VecDeque<(f64, Vec<GameEvent>)>,
    room_requested: bool,
}

impl GameState {
    pub fn entities(&self) -> &HashMap<EntityId, EntitySnapshot> {
        &self.buffer.entities
    }

    /// Position affichée d'une entité : interpolée, ou à défaut celle du dernier snapshot.
    pub fn pose(&self, id: EntityId) -> Option<Pose> {
        self.poses.get(&id).copied().or_else(|| {
            self.buffer.entities.get(&id).map(|e| Pose {
                x: e.x,
                y: e.y,
                facing: e.facing,
            })
        })
    }

    /// Temps serveur (ms) auquel le rendu affiche les entités.
    pub fn render_time(&self, local_now: f64) -> f64 {
        self.clock.server_now(local_now) - f64::from(INTERP_DELAY_MS)
    }

    fn rtt_or_default(&self) -> f64 {
        self.rtt.unwrap_or(DEFAULT_RTT)
    }

    /// Route un message du serveur.
    pub fn route(&mut self, msg: ServerMsg, local_now: f64) {
        match msg {
            ServerMsg::Welcome { player_id, .. } => {
                self.player_id = Some(player_id);
                self.connected = true;
                self.error = None;
            }
            ServerMsg::Error { message } => {
                warn!("serveur : {message}");
                self.status.clone_from(&message);
                self.error = Some(message);
            }
            ServerMsg::Lobby { state } => {
                self.lobby = Some(state);
                self.error = None;
            }
            ServerMsg::Draft { state } => self.draft = Some(state),
            ServerMsg::Loading { ends_at } => self.loading_ends_at = Some(ends_at),
            ServerMsg::GameStart { start } => {
                self.buffer.clear();
                self.clock.reset();
                self.pending.clear();
                self.poses.clear();
                self.predictor = MovePredictor::default();
                self.my_entity = Some(start.entity_id);
                self.result = None;
                self.self_state = None;
                self.game = Some(start);
                info!("partie lancée, entité du joueur : {:?}", self.my_entity);
            }
            ServerMsg::Snapshot { snap } => self.on_snapshot(*snap, local_now),
            ServerMsg::GameOver { result } => self.result = Some(result),
            ServerMsg::Pong { client_time, .. } => {
                if let Some(sent) = client_time {
                    let sample = (local_now - sent).max(0.0);
                    let rtt = match self.rtt {
                        None => sample,
                        Some(prev) => prev * 0.7 + sample * 0.3,
                    }
                    .round();
                    self.rtt = Some(rtt);
                }
            }
        }
    }

    fn on_snapshot(&mut self, snap: Snapshot, local_now: f64) {
        self.clock.sample(snap.server_time, local_now);
        if !self.buffer.push(&snap) {
            return;
        }
        self.snapshots += 1;
        for id in &self.buffer.removed {
            self.poses.remove(id);
        }
        // Un dash du joueur local suspend la prédiction le temps du dash.
        for ev in &snap.events {
            if let GameEvent::Dash {
                source, duration, ..
            } = ev
                && Some(*source) == self.my_entity
            {
                self.predictor.suspend(duration + 0.1);
            }
        }
        if self.my_entity.is_none() {
            self.my_entity = snap.self_state.as_ref().map(|s| s.entity_id);
        }
        self.tick = snap.tick;
        self.time = snap.time;
        self.score = Some(snap.score);
        self.self_state = snap.self_state;
        self.objective_timers = snap.objective_timers;
        if !snap.events.is_empty() {
            self.pending
                .push_back((snap.server_time, snap.events.clone()));
        }
        self.events = snap.events;
    }

    /// Ordre local : la prédiction démarre sans attendre le serveur.
    pub fn local_order(&mut self, msg: &ClientMsg) {
        let latency = self.rtt_or_default() / 1000.0;
        match msg {
            ClientMsg::Move { pos } | ClientMsg::AttackMove { pos } => {
                self.predictor.command(*pos, latency);
            }
            _ => self.predictor.cancel(),
        }
    }

    /// Recalcule les positions affichées (`dt` en secondes) et renvoie les événements atteints par
    /// le rendu, à publier.
    pub fn advance(&mut self, dt: f64, local_now: f64, due: &mut Vec<GameEvent>) {
        self.clock.update(dt * 1000.0);
        let rt = self.render_time(local_now);
        let me = self.my_entity;
        for (&id, e) in &self.buffer.entities {
            if Some(id) == me {
                continue;
            }
            let pose = self.buffer.sample(id, rt).unwrap_or(Pose {
                x: e.x,
                y: e.y,
                facing: e.facing,
            });
            self.poses.insert(id, pose);
        }
        if let Some(id) = me {
            self.predict_self(id, dt, local_now);
        }
        while self.pending.front().is_some_and(|(at, _)| *at <= rt)
            || self.pending.len() > MAX_PENDING_SNAPSHOTS
        {
            if let Some((_, events)) = self.pending.pop_front() {
                due.extend(events);
            }
        }
    }

    fn predict_self(&mut self, id: EntityId, dt: f64, local_now: f64) {
        let (Some(e), Some(last)) = (self.buffer.entities.get(&id), self.buffer.latest_sample(id))
        else {
            return;
        };
        let (vx, vy) = self.buffer.velocity(id).unwrap_or((0.0, 0.0));
        let lead =
            (self.clock.server_now(local_now) - last.t) / 1000.0 + self.rtt_or_default() / 2000.0;
        let speed = self
            .self_state
            .as_ref()
            .map_or(DEFAULT_MOVE_SPEED, |s| s.stats.move_speed);
        let prev = self.poses.get(&id).copied();
        let server = ServerView {
            x: last.x,
            y: last.y,
            vx,
            vy,
            lead,
        };
        let alive = e.dead != Some(true);
        let facing = e.facing;
        let pos = self.predictor.update(dt, server, speed, alive);
        let facing = match prev {
            Some(p) if self.predictor.moving() && (pos.x - p.x).hypot(pos.y - p.y) > 1.0 => {
                (pos.y - p.y).atan2(pos.x - p.x)
            }
            _ => facing,
        };
        self.poses.insert(
            id,
            Pose {
                x: pos.x,
                y: pos.y,
                facing,
            },
        );
    }
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        let url = std::env::var("LOV_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
        app.insert_resource(Connection::deferred(url))
            .init_resource::<Autoplay>()
            .init_resource::<GameState>()
            .add_message::<ServerEvent>()
            .add_systems(PostStartup, autoplay_connect)
            .add_systems(PreUpdate, (pump_network, update_poses).chain())
            .add_systems(Update, trace_events);
    }
}

/// Mode hors ligne (scène de charge) : mêmes ressources, sans fil réseau. Les ordres envoyés sont
/// perdus : le récepteur du canal est abandonné.
pub struct OfflinePlugin;

impl Plugin for OfflinePlugin {
    fn build(&self, app: &mut App) {
        let (outgoing, _) = mpsc::channel();
        let (_, incoming) = mpsc::channel();
        app.insert_resource(Connection::new(outgoing, incoming))
            .insert_resource(Autoplay(false))
            .init_resource::<GameState>()
            .add_message::<ServerEvent>();
    }
}

// ------------------------------------------------------------------------------------------------
// Fil réseau
// ------------------------------------------------------------------------------------------------

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

/// Signale un problème de connexion au jeu sous la forme d'un `ServerMsg` d'erreur.
fn report(to_game: &Sender<ServerMsg>, message: String) -> bool {
    to_game.send(ServerMsg::Error { message }).is_ok()
}

/// Comment s'est terminée une session WebSocket.
enum Ended {
    /// Le jeu s'est arrêté : le fil aussi.
    GameGone,
    /// Connexion perdue : on peut retenter.
    Lost(String),
}

fn network_thread(
    url: &str,
    name: &str,
    from_game: &Receiver<ClientMsg>,
    to_game: &Sender<ServerMsg>,
) {
    let mut token: Option<String> = None;
    let mut attempts = 0usize;
    loop {
        let ended = match tungstenite::connect(url) {
            Ok((socket, _)) => {
                attempts = 0;
                session(socket, name, &mut token, from_game, to_game)
            }
            Err(e) => Ended::Lost(format!("Connexion à {url} impossible : {e}")),
        };
        let Ended::Lost(reason) = ended else { return };
        if !report(to_game, reason) {
            return;
        }
        let delay = RECONNECT_DELAYS[attempts.min(RECONNECT_DELAYS.len() - 1)];
        attempts += 1;
        thread::sleep(Duration::from_millis(delay));
    }
}

/// Une connexion : `hello`, puis relais des messages dans les deux sens jusqu'à la coupure.
fn session(
    mut socket: Socket,
    name: &str,
    token: &mut Option<String>,
    from_game: &Receiver<ClientMsg>,
    to_game: &Sender<ServerMsg>,
) -> Ended {
    // Lecture à délai court : la même boucle sert les envois et les réceptions.
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(2)));
        let _ = stream.set_nodelay(true);
    }
    let hello = ClientMsg::Hello {
        name: name.to_owned(),
        version: PROTOCOL_VERSION,
        reconnect_token: token.clone(),
    };
    if !send_msg(&mut socket, &hello) {
        return Ended::Lost("Connexion fermée pendant le hello".into());
    }
    let mut last_ping: Option<Instant> = None;
    loop {
        if last_ping.is_none_or(|t| t.elapsed() >= PING_EVERY) {
            last_ping = Some(Instant::now());
            let ping = ClientMsg::NetPing {
                client_time: now_ms(),
            };
            if !send_msg(&mut socket, &ping) {
                return Ended::Lost("Connexion perdue".into());
            }
        }
        loop {
            match from_game.try_recv() {
                Ok(msg) => {
                    if !send_msg(&mut socket, &msg) {
                        return Ended::Lost("Connexion perdue".into());
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ended::GameGone,
            }
        }
        match socket.read() {
            // Donnée venue du réseau : un message illisible est journalisé puis ignoré.
            Ok(Message::Text(raw)) => match decode::<ServerMsg>(raw.as_str()) {
                Ok(msg) => {
                    if let ServerMsg::Welcome {
                        reconnect_token, ..
                    } = &msg
                    {
                        *token = Some(reconnect_token.clone());
                    }
                    if to_game.send(msg).is_err() {
                        return Ended::GameGone;
                    }
                }
                Err(e) => warn!("message serveur illisible, ignoré : {e}"),
            },
            Ok(Message::Close(_)) => return Ended::Lost("Le serveur a fermé la connexion".into()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Ended::Lost(format!("Connexion perdue : {e}")),
        }
    }
}

/// Encode et envoie un message ; `false` si la connexion est tombée.
fn send_msg(socket: &mut Socket, msg: &ClientMsg) -> bool {
    match encode(msg) {
        Ok(raw) => socket.send(Message::text(raw)).is_ok(),
        Err(e) => {
            warn!("message client non encodable, ignoré : {e}");
            true
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Systèmes
// ------------------------------------------------------------------------------------------------

/// Pilote automatique : connexion au lancement sous `LOV_NAME`.
fn autoplay_connect(autoplay: Res<Autoplay>, conn: Res<Connection>, mut state: ResMut<GameState>) {
    if autoplay.0 {
        let name = std::env::var("LOV_NAME").unwrap_or_else(|_| "Natif".to_string());
        conn.connect(&name);
        state.status = "Connexion…".into();
    }
}

fn pump_network(conn: Res<Connection>, autoplay: Res<Autoplay>, mut state: ResMut<GameState>) {
    if let Ok(mut orders) = conn.local_orders.lock() {
        for msg in orders.drain(..) {
            state.local_order(&msg);
        }
    }
    // Le verrou n'est jamais disputé : ce système est le seul lecteur.
    let Ok(incoming) = conn.incoming.lock() else {
        return;
    };
    let now = now_ms();
    while let Ok(msg) = incoming.try_recv() {
        let status = if autoplay.0 {
            autoplay_step(&conn, &mut state, &msg)
        } else {
            None
        };
        state.route(msg, now);
        if let Some(status) = status {
            state.status = status;
        }
    }
}

fn update_poses(
    mut state: ResMut<GameState>,
    time: Res<Time>,
    mut events: MessageWriter<ServerEvent>,
    mut due: Local<Vec<GameEvent>>,
) {
    due.clear();
    state.advance(f64::from(time.delta_secs()), now_ms(), &mut due);
    for ev in due.drain(..) {
        events.write(ServerEvent(ev));
    }
}

/// Trace des événements de jeu publiés (`RUST_LOG=lov_client=debug`), en attendant les FX.
fn trace_events(mut events: MessageReader<ServerEvent>) {
    for ServerEvent(ev) in events.read() {
        debug!("événement : {ev:?}");
    }
}

/// Pilote automatique : crée une salle de dev, la lance, choisit un champion libre. Renvoie la ligne
/// d'état à afficher.
fn autoplay_step(conn: &Connection, state: &mut GameState, msg: &ServerMsg) -> Option<String> {
    match msg {
        ServerMsg::Welcome { .. } => {
            // Une reconnexion reprend la salle : on ne recrée rien.
            if state.lobby.is_some() || state.game.is_some() {
                return Some("Reconnecté".into());
            }
            conn.send(ClientMsg::CreateRoom {
                dev_mode: Some(true),
            });
            Some("Création de la salle…".into())
        }
        ServerMsg::Lobby { state: lobby } => {
            if lobby.phase == RoomPhase::Lobby && !state.room_requested {
                state.room_requested = true;
                conn.send(ClientMsg::SetReady { ready: true });
                conn.send(ClientMsg::StartGame);
                return Some("Salle créée, lancement…".into());
            }
            None
        }
        ServerMsg::Draft { state: draft } => {
            lock_first_free_champion(conn, state.player_id.as_deref(), draft);
            Some("Draft…".into())
        }
        ServerMsg::Loading { .. } => Some("Chargement…".into()),
        ServerMsg::GameStart { .. } => Some("En jeu".into()),
        ServerMsg::GameOver { .. } => Some("Partie terminée".into()),
        _ => None,
    }
}

fn lock_first_free_champion(conn: &Connection, me: Option<&str>, draft: &DraftState) {
    let Some(me) = me else { return };
    let my_turn = draft
        .turns
        .get(draft.current_turn as usize)
        .is_some_and(|turn| turn.iter().any(|p| p == me));
    let locked = draft.picks.iter().any(|p| p.player_id == me && p.locked);
    if !my_turn || locked {
        return;
    }
    let free = CHAMPIONS.iter().map(|c| c.id.as_str()).find(|id| {
        !draft
            .picks
            .iter()
            .any(|p| p.champion_id.as_deref() == Some(*id))
    });
    if let Some(id) = free {
        conn.send(ClientMsg::DraftLock {
            champion_id: id.to_owned(),
        });
    }
}

#[cfg(test)]
mod tests;
