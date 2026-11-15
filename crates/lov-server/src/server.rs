//! Serveur HTTP (build client + `/health`) et WebSocket (`/ws`), port de `net/server.ts`.
//!
//! Sessions : chaque connexion reçoit un `reconnectToken` SECRET (différent du `playerId` public) ; un
//! `hello` avec ce token reprend le même joueur et sa salle (reconnexion en cours de partie).

use crate::room::{Conn, EMPTY_ROOM_GRACE, Room, RoomFactory};
use crate::validate::{RateLimiter, js_slice, js_trim, parse_client_msg};
use axum::{
    Router,
    extract::{
        Request, State,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use lov_shared::protocol::{ClientMsg, PROTOCOL_VERSION, ServerMsg, WS_PATH, encode};
use std::{
    collections::HashMap,
    io,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, watch},
    task::JoinHandle,
};
use tower_http::services::ServeDir;
use uuid::Uuid;

/// Messages / s autorisés par connexion (moyenne) et rafale. Au-delà de `MAX_STRIKES` dépassements : déconnexion.
pub const MSG_RATE: f64 = 40.0;
pub const MSG_BURST: f64 = 80.0;
pub const MAX_STRIKES: u32 = 400;
/// Taille maximale d'un message WebSocket (`maxPayload` du TS).
pub const MAX_PAYLOAD: usize = 8 * 1024;

const ROOM_CODE_LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

pub struct GameServerOptions {
    /// Port TCP ; 0 = port éphémère (tests).
    pub port: u16,
    pub dev_lobby: bool,
    /// Partie lancée sans draft (picks automatiques).
    pub skip_draft: bool,
    /// Dossier du build client servi en HTTP.
    pub client_dist: Option<PathBuf>,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Default for GameServerOptions {
    fn default() -> Self {
        Self {
            port: 3000,
            dev_lobby: false,
            skip_draft: false,
            client_dist: None,
            log: Arc::new(|m| println!("{m}")),
        }
    }
}

#[derive(Clone)]
struct RoomRef {
    code: String,
    id: u64,
}

struct Session {
    player_id: String,
    name: String,
    room: Option<RoomRef>,
}

type SharedSession = Arc<Mutex<Session>>;

struct RoomEntry {
    id: u64,
    room: Box<dyn Room>,
}

#[derive(Default)]
struct Registry {
    rooms: HashMap<String, RoomEntry>,
    /// token secret -> session
    sessions: HashMap<String, SharedSession>,
    next_room_id: u64,
}

struct Shared {
    state: Mutex<Registry>,
    opts: GameServerOptions,
    factory: Arc<dyn RoomFactory>,
    started: Instant,
    next_conn: AtomicU64,
    shutdown: watch::Sender<bool>,
}

/// Un verrou empoisonné (panique dans une salle) ne doit pas faire tomber toutes les connexions.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_millis() as f64)
}

fn send(conn: &Conn, msg: &ServerMsg) {
    if let Ok(raw) = encode(msg) {
        conn.send(raw);
    }
}

fn send_error(conn: &Conn, message: &str) {
    send(
        conn,
        &ServerMsg::Error {
            message: message.to_string(),
        },
    );
}

pub struct GameServer {
    addr: SocketAddr,
    shared: Arc<Shared>,
    tasks: Vec<JoinHandle<()>>,
}

impl GameServer {
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Nombre de salles ouvertes.
    pub fn room_count(&self) -> usize {
        lock(&self.shared.state).rooms.len()
    }

    /// Arrête les salles, coupe toutes les connexions et libère le port.
    pub async fn close(self) {
        for t in &self.tasks {
            t.abort();
        }
        {
            let mut st = lock(&self.shared.state);
            for e in st.rooms.values_mut() {
                e.room.stop();
            }
        }
        // Les tâches de connexion se terminent à ce signal (équivalent de `client.terminate()`).
        let _ = self.shared.shutdown.send(true);
        for t in self.tasks {
            let _ = t.await;
        }
    }
}

/// Démarre le serveur : écoute sur `opts.port` (IPv6 double pile si possible, sinon IPv4).
pub async fn create_game_server(
    opts: GameServerOptions,
    factory: Arc<dyn RoomFactory>,
) -> io::Result<GameServer> {
    let listener = match TcpListener::bind(("::", opts.port)).await {
        Ok(l) => l,
        Err(_) => TcpListener::bind(("0.0.0.0", opts.port)).await?,
    };
    let addr = listener.local_addr()?;
    let (shutdown, _) = watch::channel(false);
    let client_dist = opts.client_dist.clone();
    let shared = Arc::new(Shared {
        state: Mutex::new(Registry::default()),
        opts,
        factory,
        started: Instant::now(),
        next_conn: AtomicU64::new(1),
        shutdown,
    });

    let app = Router::new()
        .route("/health", get(health))
        .route(WS_PATH, get(ws_upgrade))
        .fallback_service(static_files(client_dist))
        .with_state(Arc::clone(&shared));

    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let janitor = tokio::spawn(janitor(Arc::clone(&shared)));
    Ok(GameServer {
        addr,
        shared,
        tasks: vec![server, janitor],
    })
}

// ------------------------------------------------------------------ HTTP

async fn health(State(shared): State<Arc<Shared>>) -> Response {
    let (rooms, players) = {
        let st = lock(&shared.state);
        let players: usize = st.rooms.values().map(|e| e.room.connected_players()).sum();
        (st.rooms.len(), players)
    };
    let body = serde_json::json!({
        "ok": true,
        "uptime": shared.started.elapsed().as_secs_f64().round() as u64,
        "rooms": rooms,
        "players": players,
    });
    (
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

/// Build du client : fichier demandé s'il existe, sinon `index.html` (SPA) ; assets versionnés en cache long.
fn static_files(dist: Option<PathBuf>) -> Router {
    let Some(dist) = dist else {
        return Router::new().fallback(|| async { (StatusCode::NOT_FOUND, "Pas de client") });
    };
    let index = dist.join("index.html");
    let spa = move || {
        let index = index.clone();
        async move {
            match tokio::fs::read(&index).await {
                Ok(bytes) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], bytes).into_response(),
                Err(_) => (
                    StatusCode::NOT_FOUND,
                    "Client non construit : lancez `npm run build` ou utilisez `npm run dev` (Vite sur :5173).",
                )
                    .into_response(),
            }
        }
    };
    let service = ServeDir::new(dist).fallback(Router::new().fallback(spa));
    Router::new()
        .fallback_service(service)
        .layer(middleware::from_fn(cache_assets))
}

async fn cache_assets(req: Request, next: Next) -> Response {
    let is_asset = req.uri().path().starts_with("/assets/");
    let mut res = next.run(req).await;
    let is_html = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if is_asset && res.status() == StatusCode::OK && !is_html {
        res.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    res
}

// ------------------------------------------------------------------ nettoyage des salles

/// Supprime les salles vides : tout de suite au lobby, après `EMPTY_ROOM_GRACE` sinon.
async fn janitor(shared: Arc<Shared>) {
    let period = Duration::from_secs(5);
    let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    loop {
        tick.tick().await;
        let now = Instant::now();
        let mut st = lock(&shared.state);
        let dead: Vec<(String, u64)> = st
            .rooms
            .iter()
            .filter(|(_, e)| {
                e.room.is_empty()
                    && (e.room.is_lobby()
                        || e.room
                            .empty_since()
                            .is_some_and(|s| now.saturating_duration_since(s) > EMPTY_ROOM_GRACE))
            })
            .map(|(code, e)| (code.clone(), e.id))
            .collect();
        for (code, id) in dead {
            if let Some(mut e) = st.rooms.remove(&code) {
                e.room.stop();
            }
            st.sessions
                .retain(|_, s| lock(s).room.as_ref().is_none_or(|r| r.id != id));
        }
    }
}

// ------------------------------------------------------------------ WebSocket

async fn ws_upgrade(ws: WebSocketUpgrade, State(shared): State<Arc<Shared>>) -> Response {
    ws.max_message_size(MAX_PAYLOAD)
        .max_frame_size(MAX_PAYLOAD)
        .on_upgrade(move |socket| handle_socket(socket, shared))
}

/// État propre à une connexion.
#[derive(Default)]
struct ConnState {
    token: Option<String>,
    session: Option<SharedSession>,
}

async fn handle_socket(mut socket: WebSocket, shared: Arc<Shared>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let conn = Conn::new(shared.next_conn.fetch_add(1, Ordering::Relaxed), tx);
    let mut limiter = RateLimiter::new(MSG_RATE, MSG_BURST);
    let mut cs = ConnState::default();
    let mut shutdown = shared.shutdown.subscribe();

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            out = rx.recv() => {
                let Some(raw) = out else { break };
                if socket.send(Message::Text(raw.into())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => {
                let raw = match incoming {
                    Some(Ok(Message::Text(t))) => t.as_str().to_owned(),
                    Some(Ok(Message::Binary(b))) => String::from_utf8_lossy(&b).into_owned(),
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                    Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                };
                if !limiter.take() {
                    if limiter.strikes > MAX_STRIKES {
                        let frame = CloseFrame { code: 1008, reason: "spam".into() };
                        let _ = socket.send(Message::Close(Some(frame))).await;
                        break;
                    }
                    continue;
                }
                let Some(msg) = parse_client_msg(&raw) else { continue };
                handle_msg(&shared, &conn, &mut cs, msg);
            }
        }
    }
    on_close(&shared, &conn, &cs);
}

/// Résout la salle référencée par une session (elle a pu être supprimée entre-temps).
fn room_of<'a>(st: &'a mut Registry, r: &RoomRef) -> Option<&'a mut Box<dyn Room>> {
    st.rooms
        .get_mut(&r.code)
        .filter(|e| e.id == r.id)
        .map(|e| &mut e.room)
}

fn new_code(rooms: &HashMap<String, RoomEntry>) -> String {
    loop {
        let bytes = *Uuid::new_v4().as_bytes();
        let code: String = bytes
            .iter()
            .take(5)
            .map(|b| char::from(ROOM_CODE_LETTERS[usize::from(*b) % ROOM_CODE_LETTERS.len()]))
            .collect();
        if !rooms.contains_key(&code) {
            return code;
        }
    }
}

fn handle_msg(shared: &Shared, conn: &Conn, cs: &mut ConnState, msg: ClientMsg) {
    match msg {
        ClientMsg::NetPing { client_time } => send(
            conn,
            &ServerMsg::Pong {
                server_time: now_ms(),
                client_time: Some(client_time),
            },
        ),
        ClientMsg::Hello {
            name,
            version,
            reconnect_token,
        } => handle_hello(shared, conn, cs, &name, version, reconnect_token),
        other => {
            let Some(session) = cs.session.clone() else {
                return send_error(conn, "hello attendu");
            };
            handle_in_session(shared, conn, &session, other);
        }
    }
}

fn handle_hello(
    shared: &Shared,
    conn: &Conn,
    cs: &mut ConnState,
    name: &str,
    version: u32,
    reconnect_token: Option<String>,
) {
    let name = js_slice(js_trim(name), 16);
    let name = if name.is_empty() { "Invocateur" } else { name };
    let mut st = lock(&shared.state);
    let prev = reconnect_token
        .as_ref()
        .and_then(|t| st.sessions.get(t).cloned());
    let (token, session) = match (&prev, reconnect_token) {
        (Some(prev), Some(token)) => (token, Arc::clone(prev)),
        _ => {
            let token = Uuid::new_v4().to_string();
            let session = Arc::new(Mutex::new(Session {
                player_id: Uuid::new_v4().to_string()[..8].to_string(),
                name: name.to_string(),
                room: None,
            }));
            st.sessions.insert(token.clone(), Arc::clone(&session));
            (token, session)
        }
    };
    let (player_id, player_name, room_ref) = {
        let s = lock(&session);
        (s.player_id.clone(), s.name.clone(), s.room.clone())
    };
    send(
        conn,
        &ServerMsg::Welcome {
            player_id: player_id.clone(),
            reconnect_token: token.clone(),
            version: PROTOCOL_VERSION,
        },
    );
    if version != PROTOCOL_VERSION {
        send_error(
            conn,
            "Version du client différente du serveur : recharge la page.",
        );
    }
    // Reprise de la salle (reconnexion).
    if prev.is_some() {
        let resumed = room_ref
            .as_ref()
            .and_then(|r| room_of(&mut st, r))
            // Le TS ignore le résultat du `join` de reprise.
            .map(|room| {
                let _ = room.join(&player_id, &player_name, conn.clone());
            })
            .is_some();
        if !resumed {
            lock(&session).room = None;
        }
    }
    cs.token = Some(token);
    cs.session = Some(session);
}

fn handle_in_session(shared: &Shared, conn: &Conn, session: &SharedSession, msg: ClientMsg) {
    let mut st = lock(&shared.state);
    let (player_id, player_name, current) = {
        let s = lock(session);
        (s.player_id.clone(), s.name.clone(), s.room.clone())
    };
    match msg {
        ClientMsg::CreateRoom { dev_mode } => {
            let code = new_code(&st.rooms);
            let dev = shared.opts.dev_lobby || dev_mode == Some(true);
            let mut room = match shared.factory.create(&code, dev, shared.opts.skip_draft) {
                Ok(room) => room,
                Err(message) => return send_error(conn, &message),
            };
            if let Some(room) = current.as_ref().and_then(|r| room_of(&mut st, r)) {
                room.leave(&player_id);
            }
            // Le TS ignore le résultat de ce `join` (une salle neuve accepte toujours son créateur).
            let _ = room.join(&player_id, &player_name, conn.clone());
            st.next_room_id += 1;
            let id = st.next_room_id;
            st.rooms.insert(code.clone(), RoomEntry { id, room });
            (shared.opts.log)(&format!(
                "[lov] salle {code} créée{}",
                if dev { " (dev)" } else { "" }
            ));
            lock(session).room = Some(RoomRef { code, id });
        }
        ClientMsg::JoinRoom { code } => {
            let code = js_trim(&code).to_uppercase();
            let Some(target_id) = st.rooms.get(&code).map(|e| e.id) else {
                return send_error(conn, "Salle introuvable");
            };
            if current.as_ref().map(|r| r.id) != Some(target_id)
                && let Some(room) = current.as_ref().and_then(|r| room_of(&mut st, r))
            {
                room.leave(&player_id);
            }
            let Some(entry) = st.rooms.get_mut(&code) else {
                return;
            };
            if let Err(message) = entry.room.join(&player_id, &player_name, conn.clone()) {
                return send_error(conn, &message);
            }
            lock(session).room = Some(RoomRef {
                code,
                id: target_id,
            });
        }
        ClientMsg::LeaveRoom => {
            if let Some(room) = current.as_ref().and_then(|r| room_of(&mut st, r)) {
                room.leave(&player_id);
            }
            lock(session).room = None;
        }
        other => {
            if let Some(room) = current.as_ref().and_then(|r| room_of(&mut st, r)) {
                room.handle(&player_id, other);
            }
        }
    }
}

fn on_close(shared: &Shared, conn: &Conn, cs: &ConnState) {
    let Some(session) = &cs.session else {
        return;
    };
    let mut st = lock(&shared.state);
    let (player_id, room_ref) = {
        let s = lock(session);
        (s.player_id.clone(), s.room.clone())
    };
    let Some(room_ref) = room_ref else {
        // Session hors salle : rien à reprendre, on libère le token.
        if let Some(token) = &cs.token {
            st.sessions.remove(token);
        }
        return;
    };
    // On ne quitte la salle que si cette connexion est toujours celle du joueur (pas d'onglet plus récent).
    if let Some(room) = room_of(&mut st, &room_ref)
        && room.is_connection_of(&player_id, conn)
    {
        room.leave(&player_id);
    }
}
