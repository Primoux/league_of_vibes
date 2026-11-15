//! Tests d'intégration du serveur : vrai WebSocket sur un port éphémère.
//!
//! Portage des parties de `net.test.ts` qui exercent `net/server.ts` sans `room.ts`. Les tests TS
//! « création et jonction de salle » et « reconnexion pendant le draft » dépendent de `Room` : ils restent à
//! porter avec les salles. Les tests « salle simulée » ci-dessous couvrent l'aiguillage du serveur avec une
//! salle factice.

use futures_util::{SinkExt, StreamExt};
use lov_server::room::{Conn, Room, RoomFactory, UnportedRooms};
use lov_server::server::{GameServer, GameServerOptions, create_game_server};
use lov_shared::protocol::{
    ClientMsg, LobbyPlayer, LobbyState, PROTOCOL_VERSION, RoomPhase, ServerMsg, WS_PATH, decode,
    encode,
};
use lov_shared::types::Team;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

fn quiet() -> GameServerOptions {
    GameServerOptions {
        port: 0,
        log: Arc::new(|_| {}),
        ..GameServerOptions::default()
    }
}

async fn start() -> GameServer {
    create_game_server(quiet(), Arc::new(UnportedRooms))
        .await
        .expect("le serveur démarre")
}

async fn connect(server: &GameServer) -> Ws {
    let url = format!("ws://127.0.0.1:{}{WS_PATH}", server.port());
    connect_async(url).await.expect("connexion WebSocket").0
}

async fn send_raw(ws: &mut Ws, raw: &str) {
    ws.send(Message::text(raw)).await.expect("envoi");
}

async fn send_msg(ws: &mut Ws, msg: &ClientMsg) {
    send_raw(ws, &encode(msg).expect("encodage")).await;
}

/// Prochain message texte du serveur (3 s max).
async fn recv_raw(ws: &mut Ws) -> String {
    loop {
        let m = timeout(Duration::from_secs(3), ws.next())
            .await
            .expect("timeout en attente d'un message")
            .expect("flux fermé")
            .expect("erreur WebSocket");
        if let Message::Text(t) = m {
            return t.to_string();
        }
    }
}

async fn recv(ws: &mut Ws) -> ServerMsg {
    let raw = recv_raw(ws).await;
    decode(&raw).unwrap_or_else(|e| panic!("message serveur illisible {raw:?} : {e}"))
}

/// Vrai si aucun message n'arrive pendant `ms`.
async fn silent(ws: &mut Ws, ms: u64) -> bool {
    timeout(Duration::from_millis(ms), ws.next()).await.is_err()
}

fn hello(name: &str, version: u32, token: Option<&str>) -> ClientMsg {
    ClientMsg::Hello {
        name: name.into(),
        version,
        reconnect_token: token.map(String::from),
    }
}

async fn handshake(ws: &mut Ws, name: &str) -> (String, String) {
    send_msg(ws, &hello(name, PROTOCOL_VERSION, None)).await;
    match recv(ws).await {
        ServerMsg::Welcome {
            player_id,
            reconnect_token,
            version,
        } => {
            assert_eq!(version, PROTOCOL_VERSION);
            (player_id, reconnect_token)
        }
        other => panic!("welcome attendu, reçu {other:?}"),
    }
}

async fn http_get(port: u16, path: &str) -> (String, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).await.expect("tcp");
    let req = format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).await.expect("écriture");
    let mut buf = String::new();
    s.read_to_string(&mut buf).await.expect("lecture");
    let (head, body) = buf.split_once("\r\n\r\n").unwrap_or((&buf, ""));
    (head.to_string(), body.to_string())
}

// net.test.ts › « serveur WebSocket » › « /health »
#[tokio::test]
async fn health() {
    let server = start().await;
    let (head, body) = http_get(server.port(), "/health").await;
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(
        head.to_lowercase()
            .contains("content-type: application/json")
    );
    let v: serde_json::Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(v["ok"], true);
    assert_eq!(v["rooms"], 0);
    assert_eq!(v["players"], 0);
    assert!(v["uptime"].is_u64());
    server.close().await;
}

#[tokio::test]
async fn sans_client_les_autres_chemins_font_404() {
    let server = start().await;
    let (head, body) = http_get(server.port(), "/").await;
    assert!(head.starts_with("HTTP/1.1 404"), "{head}");
    assert_eq!(body, "Pas de client");
    server.close().await;
}

// net.test.ts › « hello → welcome avec token secret » (partie sans salle) + ping réseau
#[tokio::test]
async fn hello_welcome_token_secret_et_ping_reseau() {
    let server = start().await;
    let mut a = connect(&server).await;
    let (player_id, token) = handshake(&mut a, "Alice").await;
    assert_ne!(token, player_id);
    assert_eq!(player_id.len(), 8);

    send_msg(&mut a, &ClientMsg::NetPing { client_time: 42.0 }).await;
    match recv(&mut a).await {
        ServerMsg::Pong {
            client_time,
            server_time,
        } => {
            assert_eq!(client_time, Some(42.0));
            assert!(server_time > 1.6e12);
        }
        other => panic!("pong attendu, reçu {other:?}"),
    }
    server.close().await;
}

// netPing est traité avant `hello` (server.ts)
#[tokio::test]
async fn net_ping_sans_hello() {
    let server = start().await;
    let mut a = connect(&server).await;
    send_raw(&mut a, r#"{"t":"netPing","clientTime":7}"#).await;
    let raw = recv_raw(&mut a).await;
    let v: serde_json::Value = serde_json::from_str(&raw).expect("JSON");
    assert_eq!(v["t"], "pong");
    assert_eq!(v["clientTime"].as_f64(), Some(7.0));
    server.close().await;
}

#[tokio::test]
async fn message_invalide_ignore_et_hello_attendu() {
    let server = start().await;
    let mut a = connect(&server).await;
    // invalides : ignorés en silence, la connexion reste ouverte
    send_raw(&mut a, "pas du json").await;
    send_raw(&mut a, r#"{"t":"move","pos":{"x":"a","y":1}}"#).await;
    send_raw(&mut a, r#"{"t":"inconnu"}"#).await;
    assert!(silent(&mut a, 150).await);
    // valide mais sans session
    send_raw(&mut a, r#"{"t":"createRoom"}"#).await;
    assert_eq!(
        recv_raw(&mut a).await,
        r#"{"t":"error","message":"hello attendu"}"#
    );
    // la connexion fonctionne toujours
    handshake(&mut a, "Alice").await;
    server.close().await;
}

#[tokio::test]
async fn mauvaise_version_de_protocole() {
    let server = start().await;
    let mut a = connect(&server).await;
    send_msg(&mut a, &hello("Alice", PROTOCOL_VERSION + 1, None)).await;
    assert!(matches!(recv(&mut a).await, ServerMsg::Welcome { .. }));
    assert_eq!(
        recv_raw(&mut a).await,
        r#"{"t":"error","message":"Version du client différente du serveur : recharge la page."}"#
    );
    server.close().await;
}

#[tokio::test]
async fn nom_nettoye() {
    // Le nom n'est visible qu'à travers une salle : on le vérifie avec la salle simulée (voir plus bas).
    let (server, log) = start_mock().await;
    let mut a = connect(&server).await;
    send_msg(&mut a, &hello("   \u{feff}  ", PROTOCOL_VERSION, None)).await;
    recv(&mut a).await;
    send_msg(&mut a, &ClientMsg::CreateRoom { dev_mode: None }).await;
    recv(&mut a).await;
    let mut b = connect(&server).await;
    send_msg(
        &mut b,
        &hello(&format!("  {}  ", "x".repeat(40)), PROTOCOL_VERSION, None),
    )
    .await;
    recv(&mut b).await;
    send_msg(&mut b, &ClientMsg::CreateRoom { dev_mode: None }).await;
    recv(&mut b).await;
    let log = log.lock().unwrap().clone();
    assert!(log.iter().any(|l| l.ends_with(":Invocateur")), "{log:?}");
    assert!(
        log.iter()
            .any(|l| l.ends_with(&format!(":{}", "x".repeat(16)))),
        "{log:?}"
    );
    server.close().await;
}

// net.test.ts › « reconnexion » (partie sans salle) : usurper le playerId public ne donne pas la session
#[tokio::test]
async fn usurper_le_player_id_ne_donne_pas_la_session() {
    let server = start().await;
    let mut a = connect(&server).await;
    let (player_id, token) = handshake(&mut a, "Alice").await;

    let mut c = connect(&server).await;
    send_msg(&mut c, &hello("Eve", PROTOCOL_VERSION, Some(&player_id))).await;
    match recv(&mut c).await {
        ServerMsg::Welcome { player_id: p, .. } => assert_ne!(p, player_id),
        other => panic!("{other:?}"),
    }

    // Le vrai token reprend la même identité.
    let mut a2 = connect(&server).await;
    send_msg(&mut a2, &hello("Alice", PROTOCOL_VERSION, Some(&token))).await;
    match recv(&mut a2).await {
        ServerMsg::Welcome {
            player_id: p,
            reconnect_token,
            ..
        } => {
            assert_eq!(p, player_id);
            assert_eq!(reconnect_token, token);
        }
        other => panic!("{other:?}"),
    }
    server.close().await;
}

// Une session sans salle libère son token à la déconnexion (server.ts, `close`).
#[tokio::test]
async fn token_libere_a_la_deconnexion_hors_salle() {
    let server = start().await;
    let mut a = connect(&server).await;
    let (player_id, token) = handshake(&mut a, "Alice").await;
    a.close(None).await.expect("fermeture");
    drop(a);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut a2 = connect(&server).await;
    send_msg(&mut a2, &hello("Alice", PROTOCOL_VERSION, Some(&token))).await;
    match recv(&mut a2).await {
        ServerMsg::Welcome { player_id: p, .. } => assert_ne!(p, player_id),
        other => panic!("{other:?}"),
    }
    server.close().await;
}

#[tokio::test]
async fn creation_de_salle_refusee_par_le_placeholder() {
    let server = start().await;
    let mut a = connect(&server).await;
    handshake(&mut a, "Alice").await;
    send_msg(
        &mut a,
        &ClientMsg::CreateRoom {
            dev_mode: Some(true),
        },
    )
    .await;
    match recv(&mut a).await {
        ServerMsg::Error { message } => {
            assert!(message.contains("pas encore portées"), "{message}")
        }
        other => panic!("{other:?}"),
    }
    send_msg(
        &mut a,
        &ClientMsg::JoinRoom {
            code: "ABCDE".into(),
        },
    )
    .await;
    assert_eq!(
        recv_raw(&mut a).await,
        r#"{"t":"error","message":"Salle introuvable"}"#
    );
    server.close().await;
}

#[tokio::test]
async fn message_trop_gros_ferme_la_connexion() {
    let server = start().await;
    let mut a = connect(&server).await;
    handshake(&mut a, "Alice").await;
    send_raw(&mut a, &"x".repeat(9 * 1024)).await;
    let end = timeout(Duration::from_secs(3), async {
        loop {
            match a.next().await {
                None | Some(Err(_) | Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert!(end.is_ok(), "la connexion aurait dû être fermée");
    server.close().await;
}

#[tokio::test]
async fn spam_ferme_la_connexion_avec_1008() {
    let server = start().await;
    let mut a = connect(&server).await;
    handshake(&mut a, "Alice").await;
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut code = None;
    'outer: while Instant::now() < deadline {
        for _ in 0..200 {
            if a.send(Message::text(r#"{"t":"stop"}"#)).await.is_err() {
                break;
            }
        }
        while let Ok(next) = timeout(Duration::from_millis(5), a.next()).await {
            match next {
                Some(Ok(Message::Close(Some(f)))) => {
                    code = Some((u16::from(f.code), f.reason.to_string()));
                    break 'outer;
                }
                None | Some(Err(_) | Ok(Message::Close(None))) => break 'outer,
                Some(Ok(_)) => {}
            }
        }
    }
    assert_eq!(code, Some((1008, "spam".to_string())));
    server.close().await;
}

// ------------------------------------------------------------------ salle simulée

type Log = Arc<Mutex<Vec<String>>>;

struct MockRoom {
    code: String,
    log: Log,
    players: Vec<(String, Conn)>,
}

impl Room for MockRoom {
    fn join(&mut self, player_id: &str, name: &str, conn: Conn) -> Result<(), String> {
        if name == "Plein" {
            return Err("Salle pleine".into());
        }
        self.log
            .lock()
            .unwrap()
            .push(format!("join:{player_id}:{name}"));
        self.players.retain(|(p, _)| p != player_id);
        self.players.push((player_id.to_string(), conn));
        let state = LobbyState {
            code: self.code.clone(),
            phase: RoomPhase::Lobby,
            players: self
                .players
                .iter()
                .map(|(id, _)| LobbyPlayer {
                    id: id.clone(),
                    name: name.to_string(),
                    team: Team::Blue,
                    ready: false,
                    is_host: false,
                    connected: true,
                    role: None,
                })
                .collect(),
            dev_mode: false,
        };
        let raw = encode(&ServerMsg::Lobby { state }).unwrap();
        for (_, c) in &self.players {
            c.send(raw.clone());
        }
        Ok(())
    }
    fn leave(&mut self, player_id: &str) {
        self.log.lock().unwrap().push(format!("leave:{player_id}"));
        self.players.retain(|(p, _)| p != player_id);
    }
    fn handle(&mut self, player_id: &str, msg: ClientMsg) {
        self.log
            .lock()
            .unwrap()
            .push(format!("handle:{player_id}:{msg:?}"));
    }
    fn is_empty(&self) -> bool {
        self.players.is_empty()
    }
    fn is_lobby(&self) -> bool {
        true
    }
    fn empty_since(&self) -> Option<Instant> {
        None
    }
    fn connected_players(&self) -> usize {
        self.players.len()
    }
    fn is_connection_of(&self, player_id: &str, conn: &Conn) -> bool {
        self.players
            .iter()
            .any(|(p, c)| p == player_id && c.same_as(conn))
    }
    fn stop(&mut self) {}
}

struct MockFactory(Log);

impl RoomFactory for MockFactory {
    fn create(&self, code: &str, _: bool, _: bool) -> Result<Box<dyn Room>, String> {
        Ok(Box::new(MockRoom {
            code: code.to_string(),
            log: Arc::clone(&self.0),
            players: Vec::new(),
        }))
    }
}

async fn start_mock() -> (GameServer, Log) {
    let log: Log = Arc::default();
    let server = create_game_server(quiet(), Arc::new(MockFactory(Arc::clone(&log))))
        .await
        .expect("le serveur démarre");
    (server, log)
}

async fn lobby(ws: &mut Ws) -> LobbyState {
    match recv(ws).await {
        ServerMsg::Lobby { state } => state,
        other => panic!("lobby attendu, reçu {other:?}"),
    }
}

// net.test.ts › « hello → welcome … création et jonction de salle » avec une salle factice
#[tokio::test]
async fn creation_jonction_et_depart_de_salle() {
    let (server, log) = start_mock().await;
    let mut a = connect(&server).await;
    handshake(&mut a, "Alice").await;
    send_msg(
        &mut a,
        &ClientMsg::CreateRoom {
            dev_mode: Some(true),
        },
    )
    .await;
    let l1 = lobby(&mut a).await;
    assert_eq!(l1.code.len(), 5);
    assert_eq!(server.room_count(), 1);

    let mut b = connect(&server).await;
    handshake(&mut b, "Bob").await;
    // le code est insensible à la casse et aux espaces
    let code = format!(" {} ", l1.code.to_lowercase());
    send_msg(&mut b, &ClientMsg::JoinRoom { code }).await;
    assert_eq!(lobby(&mut b).await.players.len(), 2);
    assert_eq!(lobby(&mut a).await.players.len(), 2);

    // un message de jeu est transmis à la salle
    send_msg(&mut b, &ClientMsg::StartGame).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        log.lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("handle:") && l.ends_with("StartGame"))
    );

    // fermer une connexion fait quitter la salle
    drop(b);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(log.lock().unwrap().iter().any(|l| l.starts_with("leave:")));

    // une erreur de `join` est renvoyée telle quelle
    let mut c = connect(&server).await;
    handshake(&mut c, "Plein").await;
    send_msg(
        &mut c,
        &ClientMsg::JoinRoom {
            code: l1.code.clone(),
        },
    )
    .await;
    assert_eq!(
        recv_raw(&mut c).await,
        r#"{"t":"error","message":"Salle pleine"}"#
    );
    server.close().await;
}

// net.test.ts › « reconnexion avec le token » avec une salle factice : la salle est reprise
#[tokio::test]
async fn reconnexion_reprend_la_salle() {
    let (server, log) = start_mock().await;
    let mut a = connect(&server).await;
    let (player_id, token) = handshake(&mut a, "Alice").await;
    send_msg(&mut a, &ClientMsg::CreateRoom { dev_mode: None }).await;
    let code = lobby(&mut a).await.code;
    // Nouvelle connexion (onglet plus récent) avant la fermeture de l'ancienne.
    let mut a2 = connect(&server).await;
    send_msg(&mut a2, &hello("Alice", PROTOCOL_VERSION, Some(&token))).await;
    match recv(&mut a2).await {
        ServerMsg::Welcome { player_id: p, .. } => assert_eq!(p, player_id),
        other => panic!("{other:?}"),
    }
    assert_eq!(lobby(&mut a2).await.code, code);
    // Fermer l'ancienne connexion ne fait pas quitter la salle (ce n'est plus la connexion du joueur).
    drop(a);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(!log.lock().unwrap().iter().any(|l| l.starts_with("leave:")));
    server.close().await;
}
