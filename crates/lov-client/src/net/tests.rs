//! Tests du réseau client : port de `packages/client/test/net.test.ts`, plus la fusion des deltas et
//! des patches (protocole v2) et le routage de l'état.

use lov_shared::protocol::{EntityField, EntityPatch, Score, Snapshot, TeamScore};
use lov_shared::types::{EntityId, EntityKind, EntitySnapshot, GameEvent, TeamOrNeutral, Vec2};

use super::GameState;
use super::clock::ServerClock;
use super::prediction::{MovePredictor, SNAP_DIST, ServerView};
use super::snapshots::{MAX_EXTRAPOLATE_MS, SnapshotBuffer, lerp_angle};

fn ent(id: EntityId, x: f64, y: f64) -> EntitySnapshot {
    EntitySnapshot {
        id,
        kind: EntityKind::Minion,
        team: TeamOrNeutral::Blue,
        x,
        y,
        facing: 0.0,
        hp: 100.0,
        max_hp: 100.0,
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

fn dead(mut e: EntitySnapshot) -> EntitySnapshot {
    e.dead = Some(true);
    e
}

fn snap(
    tick: u64,
    server_time: f64,
    entities: Vec<EntitySnapshot>,
    removed: Option<Vec<EntityId>>,
) -> Snapshot {
    let team = || TeamScore {
        kills: 0,
        towers: 0,
        dragons: vec![],
        grubs: 0,
    };
    Snapshot {
        tick,
        time: tick as f64 / 30.0,
        server_time,
        entities,
        removed,
        patches: None,
        events: vec![],
        self_state: None,
        objective_timers: None,
        score: Score {
            blue: team(),
            red: team(),
        },
    }
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-3, "{a} != {b}");
}

// ------------------------------------------------------------------------------------------------
// SnapshotBuffer
// ------------------------------------------------------------------------------------------------

#[test]
fn interpole_lineairement_entre_deux_snapshots() {
    let mut b = SnapshotBuffer::default();
    b.push(&snap(1, 1000.0, vec![ent(1, 0.0, 0.0)], None));
    b.push(&snap(2, 1050.0, vec![ent(1, 100.0, 50.0)], None));
    let p = b.sample(1, 1025.0).unwrap();
    close(p.x, 50.0);
    close(p.y, 25.0);
    assert_eq!(b.sample(1, 900.0).unwrap().x, 0.0);
}

#[test]
fn extrapole_de_facon_bornee() {
    let mut b = SnapshotBuffer::default();
    b.push(&snap(1, 1000.0, vec![ent(1, 0.0, 0.0)], None));
    b.push(&snap(2, 1050.0, vec![ent(1, 100.0, 0.0)], None));
    close(b.sample(1, 1075.0).unwrap().x, 150.0);
    close(
        b.sample(1, 5000.0).unwrap().x,
        100.0 + 100.0 * MAX_EXTRAPOLATE_MS / 50.0,
    );
}

#[test]
fn ignore_les_snapshots_obsoletes_ou_dupliques() {
    let mut b = SnapshotBuffer::default();
    assert!(b.push(&snap(5, 1000.0, vec![ent(1, 0.0, 0.0)], None)));
    assert!(!b.push(&snap(4, 990.0, vec![ent(1, 999.0, 0.0)], None)));
    assert!(!b.push(&snap(5, 1000.0, vec![ent(1, 999.0, 0.0)], None)));
    assert_eq!(b.entities[&1].x, 0.0);
}

#[test]
fn snapshot_complet_supprime_les_absents() {
    let mut b = SnapshotBuffer::default();
    b.push(&snap(
        1,
        1000.0,
        vec![ent(1, 0.0, 0.0), ent(2, 0.0, 0.0)],
        None,
    ));
    b.push(&snap(2, 1050.0, vec![ent(1, 0.0, 0.0)], None));
    assert_eq!(b.entities.keys().copied().collect::<Vec<_>>(), [1]);
    assert_eq!(b.removed, [2]);
}

#[test]
fn snapshot_delta_conserve_les_absents_et_applique_removed() {
    let mut b = SnapshotBuffer::default();
    b.push(&snap(
        1,
        1000.0,
        vec![ent(1, 0.0, 0.0), ent(2, 0.0, 0.0), ent(3, 0.0, 0.0)],
        None,
    ));
    b.push(&snap(2, 1050.0, vec![ent(1, 10.0, 0.0)], Some(vec![3])));
    let mut ids: Vec<_> = b.entities.keys().copied().collect();
    ids.sort_unstable();
    assert_eq!(ids, [1, 2]);
    assert_eq!(b.entities[&1].x, 10.0);
}

#[test]
fn n_interpole_pas_les_teleportations() {
    let mut b = SnapshotBuffer::default();
    b.push(&snap(1, 1000.0, vec![ent(1, 0.0, 0.0)], None));
    b.push(&snap(2, 1050.0, vec![ent(1, 400.0, 0.0)], None));
    assert_eq!(b.sample(1, 1025.0).unwrap().x, 400.0);
    // Réapparition : mort puis vivant.
    b.push(&snap(3, 1100.0, vec![dead(ent(1, 410.0, 0.0))], None));
    b.push(&snap(4, 1150.0, vec![ent(1, 420.0, 0.0)], None));
    assert_eq!(b.sample(1, 1120.0).unwrap().x, 420.0);
}

#[test]
fn calcule_la_vitesse_serveur_recente() {
    let mut b = SnapshotBuffer::default();
    b.push(&snap(1, 1000.0, vec![ent(1, 0.0, 0.0)], None));
    b.push(&snap(2, 1100.0, vec![ent(1, 33.5, 0.0)], None));
    close(b.velocity(1).unwrap().0, 335.0);
}

#[test]
fn garde_un_historique_borne() {
    let mut b = SnapshotBuffer::default();
    for i in 0..50u64 {
        b.push(&snap(
            i + 1,
            1000.0 + i as f64 * 50.0,
            vec![ent(1, i as f64 * 10.0, 0.0)],
            None,
        ));
    }
    close(b.sample(1, 1000.0 + 48.0 * 50.0 + 25.0).unwrap().x, 485.0);
}

#[test]
fn patch_modifie_et_supprime_des_champs() {
    let mut b = SnapshotBuffer::default();
    b.push(&snap(1, 1000.0, vec![ent(1, 10.0, 5.0)], None));
    let mut patch = EntityPatch::new(1);
    patch.x = Some(12.0);
    patch.u = Some(vec![EntityField::Anim]);
    let mut delta = snap(2, 1050.0, vec![], Some(vec![]));
    delta.patches = Some(vec![patch, {
        let mut unknown = EntityPatch::new(9);
        unknown.x = Some(1.0);
        unknown
    }]);
    b.push(&delta);
    let e = &b.entities[&1];
    assert_eq!((e.x, e.y, e.anim.clone()), (12.0, 5.0, None));
    assert!(
        !b.entities.contains_key(&9),
        "patch d'une entité inconnue ignoré"
    );
}

#[test]
fn lerp_angle_prend_le_chemin_le_plus_court() {
    assert!((lerp_angle(3.0, -3.0, 0.5) - std::f64::consts::PI).abs() < 0.05);
    close(lerp_angle(0.0, 1.0, 0.5), 0.5);
}

// ------------------------------------------------------------------------------------------------
// ServerClock
// ------------------------------------------------------------------------------------------------

#[test]
fn horloge_retient_l_echantillon_le_moins_retarde() {
    let mut c = ServerClock::default();
    // Décalage réel : +5000 ms ; latences 30, 80, 20 ms.
    c.sample(10_000.0, 10_000.0 - 5000.0 + 30.0);
    c.sample(10_050.0, 10_050.0 - 5000.0 + 80.0);
    c.sample(10_100.0, 10_100.0 - 5000.0 + 20.0);
    c.update(10_000.0);
    close(c.server_now(0.0), 5000.0 - 20.0);
    close(c.server_now(0.0), 4980.0);
}

#[test]
fn horloge_lisse_les_petites_variations() {
    let mut c = ServerClock::new(2);
    c.sample(1000.0, 0.0);
    c.update(16.0);
    c.sample(1100.0, 0.0);
    c.update(100.0);
    let offset = c.server_now(0.0);
    assert!(offset > 1000.0 && offset < 1100.0);
}

// ------------------------------------------------------------------------------------------------
// MovePredictor
// ------------------------------------------------------------------------------------------------

fn open_field() -> MovePredictor {
    MovePredictor::new(|_, _| false)
}

fn still(x: f64, y: f64) -> ServerView {
    ServerView {
        x,
        y,
        ..ServerView::default()
    }
}

fn at(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

#[test]
fn avance_immediatement_vers_le_point_clique() {
    let mut p = open_field();
    p.reset(0.0, 0.0);
    p.command(at(1000.0, 0.0), 0.1);
    let pos = p.update(0.1, still(0.0, 0.0), 335.0, true);
    assert!(pos.x > 20.0 && pos.x < 33.5, "{}", pos.x);
}

#[test]
fn converge_vers_la_position_serveur_confirmee() {
    let mut p = open_field();
    p.reset(0.0, 0.0);
    p.command(at(3000.0, 0.0), 0.1);
    let mut sx = 0.0;
    for i in 0..120 {
        // Le serveur suit avec ~100 ms de retard à 335 u/s ; `lead = 0.1` compense.
        let moving = i >= 6;
        if moving {
            sx += 335.0 / 60.0;
        }
        let server = ServerView {
            x: sx,
            y: 0.0,
            vx: if moving { 335.0 } else { 0.0 },
            vy: 0.0,
            lead: 0.1,
        };
        p.update(1.0 / 60.0, server, 335.0, true);
    }
    assert!((p.pos.x - (sx + 33.5)).abs() < 15.0);
}

#[test]
fn recale_immediatement_sur_un_gros_ecart() {
    let mut p = open_field();
    p.reset(0.0, 0.0);
    let pos = p.update(0.016, still(SNAP_DIST + 100.0, 0.0), 335.0, true);
    assert_eq!(pos.x, SNAP_DIST + 100.0);
}

#[test]
fn abandonne_si_le_serveur_reste_immobile() {
    let mut p = open_field();
    p.reset(0.0, 0.0);
    p.command(at(2000.0, 0.0), 0.05);
    for _ in 0..60 {
        p.update(1.0 / 60.0, still(0.0, 0.0), 335.0, true);
    }
    assert!(!p.moving());
    for _ in 0..120 {
        p.update(1.0 / 60.0, still(0.0, 0.0), 335.0, true);
    }
    assert!(p.pos.x.abs() < 2.0);
}

#[test]
fn ne_predit_pas_a_travers_un_mur() {
    let mut p = MovePredictor::new(|_, _| true);
    p.reset(0.0, 0.0);
    p.command(at(1000.0, 0.0), 0.1);
    assert!(!p.moving());
}

#[test]
fn suspend_pendant_un_dash_et_suit_le_serveur() {
    let mut p = open_field();
    p.reset(0.0, 0.0);
    p.suspend(0.3);
    let pos = p.update(0.016, still(120.0, 40.0), 335.0, true);
    assert_eq!((pos.x, pos.y), (120.0, 40.0));
}

// ------------------------------------------------------------------------------------------------
// GameState
// ------------------------------------------------------------------------------------------------

#[test]
fn les_evenements_sont_publies_quand_le_rendu_atteint_leur_snapshot() {
    let mut state = GameState::default();
    let mut s = snap(1, 10_000.0, vec![ent(1, 0.0, 0.0)], None);
    s.events = vec![GameEvent::Respawn {
        target: 1,
        pos: at(0.0, 0.0),
    }];
    // Reçu à l'instant local 10 000 : décalage d'horloge nul.
    state.route(
        lov_shared::protocol::ServerMsg::Snapshot { snap: Box::new(s) },
        10_000.0,
    );
    assert_eq!(state.events.len(), 1, "lecture immédiate par l'interface");
    let mut due = Vec::new();
    state.advance(0.016, 10_050.0, &mut due);
    assert!(due.is_empty(), "rendu à 9 950 ms : pas encore atteint");
    state.advance(0.016, 10_150.0, &mut due);
    assert_eq!(due.len(), 1);
}

#[test]
fn les_positions_affichees_sont_interpolees() {
    let mut state = GameState::default();
    for (tick, t, x) in [(1, 1000.0, 0.0), (2, 1050.0, 100.0)] {
        state.route(
            lov_shared::protocol::ServerMsg::Snapshot {
                snap: Box::new(snap(tick, t, vec![ent(1, x, 0.0)], None)),
            },
            t,
        );
    }
    // Rendu à 1150 − 100 = 1050 ms (temps serveur) : dernière position.
    let mut due = Vec::new();
    state.advance(0.0, 1150.0, &mut due);
    close(state.pose(1).unwrap().x, 100.0);
    state.advance(0.0, 1125.0, &mut due);
    close(state.pose(1).unwrap().x, 50.0);
}

// ---------------------------------------------------------------- connexion à la demande, pilote automatique

/// App minimale avec le pompage réseau, une connexion dont on tient les deux bouts.
fn pumped_app(
    autoplay: bool,
) -> (
    bevy::prelude::App,
    std::sync::mpsc::Sender<lov_shared::protocol::ServerMsg>,
    std::sync::mpsc::Receiver<lov_shared::protocol::ClientMsg>,
) {
    use bevy::prelude::*;
    let (outgoing, sent) = std::sync::mpsc::channel();
    let (to_game, incoming) = std::sync::mpsc::channel();
    let mut app = App::new();
    app.insert_resource(super::Connection::new(outgoing, incoming))
        .insert_resource(super::Autoplay(autoplay))
        .init_resource::<GameState>()
        .add_systems(Update, super::pump_network);
    (app, to_game, sent)
}

fn welcome() -> lov_shared::protocol::ServerMsg {
    lov_shared::protocol::ServerMsg::Welcome {
        player_id: "p1".into(),
        reconnect_token: "jeton".into(),
        version: lov_shared::protocol::PROTOCOL_VERSION,
    }
}

#[test]
fn sans_pilote_automatique_le_client_ne_cree_pas_de_salle_tout_seul() {
    let (mut app, server, sent) = pumped_app(false);
    server.send(welcome()).unwrap();
    app.update();
    assert!(sent.try_recv().is_err());
    let state = app.world().resource::<GameState>();
    assert!(state.connected);
    assert_eq!(state.player_id.as_deref(), Some("p1"));
}

#[test]
fn le_pilote_automatique_cree_une_salle_de_dev() {
    let (mut app, server, sent) = pumped_app(true);
    server.send(welcome()).unwrap();
    app.update();
    assert!(matches!(
        sent.try_recv(),
        Ok(lov_shared::protocol::ClientMsg::CreateRoom {
            dev_mode: Some(true)
        })
    ));
}

#[test]
fn la_connexion_ne_part_qu_au_premier_connect() {
    // port 9 (discard) : la tentative échoue en arrière-plan, sans effet sur le test
    let conn = super::Connection::deferred("ws://127.0.0.1:9/ws".into());
    assert!(!conn.started());
    conn.send(lov_shared::protocol::ClientMsg::LeaveRoom);
    conn.connect("Alice");
    assert!(conn.started());
    // un second appel ne relance rien
    conn.connect("Bob");
    assert!(conn.started());
}

#[test]
fn adresse_du_serveur_lue_dans_le_fichier() {
    use super::url_from_file;
    assert_eq!(
        url_from_file("# commentaire\n\n  ws://jeu.exemple:3000/ws \n"),
        Some("ws://jeu.exemple:3000/ws".into())
    );
    assert_eq!(
        url_from_file("192.168.1.20:3000"),
        Some("ws://192.168.1.20:3000/ws".into())
    );
    assert_eq!(url_from_file("# rien\n"), None);
}

#[test]
fn adresses_http_https_et_sans_chemin_normalisees() {
    use super::{normalize_url, url_from_file};
    let cases = [
        ("https://lov.exemple.app", "wss://lov.exemple.app/ws"),
        ("https://lov.exemple.app/", "wss://lov.exemple.app/ws"),
        ("http://192.168.1.20:3000", "ws://192.168.1.20:3000/ws"),
        ("HTTPS://lov.exemple.app/ws", "wss://lov.exemple.app/ws"),
        ("wss://lov.exemple.app/ws", "wss://lov.exemple.app/ws"),
        ("wss://lov.exemple.app", "wss://lov.exemple.app/ws"),
        ("ws://localhost:3077/ws", "ws://localhost:3077/ws"),
        ("ws://localhost:3077", "ws://localhost:3077/ws"),
        ("localhost:3000", "ws://localhost:3000/ws"),
        ("  ws://h/autre/route  ", "ws://h/autre/route"),
    ];
    for (raw, want) in cases {
        assert_eq!(normalize_url(raw), want, "{raw}");
    }
    // plus de `ws://http://…/ws` (#272)
    assert_eq!(
        url_from_file("http://jeu.exemple:3000\n"),
        Some("ws://jeu.exemple:3000/ws".into())
    );
}

/// `wss://` passe par TLS : le client envoie un `ClientHello` (enregistrement TLS de type 0x16), au lieu
/// d'échouer sur « TLS non compilé ». Le faux serveur coupe aussitôt, la connexion échoue ensuite.
#[test]
fn une_adresse_wss_ouvre_une_poignee_de_main_tls() {
    use std::io::Read;
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").expect("port local");
    let port = listener.local_addr().expect("adresse").port();
    let server = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().expect("connexion");
        let mut first = [0u8; 1];
        sock.read_exact(&mut first).map(|()| first[0]).ok()
    });
    let Err(err) = tungstenite::connect(format!("wss://localhost:{port}/ws")) else {
        panic!("le faux serveur ne parle pas TLS");
    };
    assert!(
        !matches!(err, tungstenite::Error::Url(_)),
        "TLS indisponible : {err}"
    );
    assert_eq!(server.join().ok().flatten(), Some(0x16));
}
