//! Tests des salles : port de `packages/server/test/room.test.ts`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lov_server::room::{
    Clock, Conn, DraftTimings, FINALIZE_SECONDS, GameRoom, RoomCore, RoomOptions,
};
use lov_shared::constants::{DRAFT_TURN_SECONDS, LOADING_SECONDS};
use lov_shared::protocol::{ClientMsg, RoomPhase, ServerMsg, Snapshot, decode};
use lov_shared::types::{SummonerId, Team};
use lov_sim::systems::combat::combat_system;
use lov_sim::systems::input::input_system;
use lov_sim::systems::movement::movement_system;
use lov_sim::systems::vision::vision_system;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

/// Horloge de test (ms), avancée à la main.
#[derive(Default)]
struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now(&self) -> f64 {
        self.0.load(Ordering::Relaxed) as f64
    }
}

/// Connexion factice : garde les messages reçus.
struct FakeConn {
    rx: UnboundedReceiver<String>,
    msgs: Vec<ServerMsg>,
}

impl FakeConn {
    fn new(id: u64) -> (Conn, Self) {
        let (tx, rx) = unbounded_channel();
        (
            Conn::new(id, tx),
            Self {
                rx,
                msgs: Vec::new(),
            },
        )
    }

    fn pump(&mut self) {
        while let Ok(raw) = self.rx.try_recv() {
            self.msgs
                .push(decode(&raw).expect("message serveur valide"));
        }
    }

    fn last_error(&mut self) -> Option<String> {
        self.pump();
        self.msgs.iter().rev().find_map(|m| match m {
            ServerMsg::Error { message } => Some(message.clone()),
            _ => None,
        })
    }

    fn has_loading(&mut self) -> bool {
        self.pump();
        self.msgs
            .iter()
            .any(|m| matches!(m, ServerMsg::Loading { .. }))
    }

    fn last_draft_hover(&mut self, of: &str) -> Option<String> {
        self.pump();
        self.msgs.iter().rev().find_map(|m| match m {
            ServerMsg::Draft { state } => state
                .picks
                .iter()
                .find(|p| p.player_id == of)
                .map(|p| p.hover_champion_id.clone().unwrap_or_default()),
            _ => None,
        })
    }

    fn snapshots(&mut self) -> Vec<Snapshot> {
        self.pump();
        self.msgs
            .iter()
            .filter_map(|m| match m {
                ServerMsg::Snapshot { snap } => Some((**snap).clone()),
                _ => None,
            })
            .collect()
    }
}

struct Setup {
    room: GameRoom,
    clock: Arc<TestClock>,
    conns: Vec<(String, Arc<Mutex<FakeConn>>)>,
}

impl Setup {
    fn advance(&self, seconds: f64) {
        self.clock
            .0
            .fetch_add((seconds * 1000.0) as u64, Ordering::Relaxed);
        self.room.core().update();
    }

    fn conn(&self, id: &str) -> std::sync::MutexGuard<'_, FakeConn> {
        self.conns
            .iter()
            .find(|(p, _)| p == id)
            .map(|(_, c)| c)
            .unwrap()
            .lock()
            .unwrap()
    }

    fn handle(&self, id: &str, msg: ClientMsg) {
        self.room.core().handle(id, msg);
    }

    fn ready_all(&self) {
        let ids: Vec<String> = self.conns.iter().map(|(p, _)| p.clone()).collect();
        for id in ids {
            self.handle(&id, ClientMsg::SetReady { ready: true });
        }
    }
}

fn setup(n: usize) -> Setup {
    let clock = Arc::new(TestClock(AtomicU64::new(1_000_000)));
    let opts = RoomOptions {
        clock: clock.clone(),
        manual: true,
        seed: Some(0),
        timings: Some(DraftTimings {
            turn: DRAFT_TURN_SECONDS,
            finalize: FINALIZE_SECONDS,
            loading: LOADING_SECONDS,
        }),
        systems: Some(Arc::new(|| {
            vec![
                input_system(),
                movement_system(),
                combat_system(),
                vision_system(),
            ]
        })),
        ..RoomOptions::default()
    };
    let room = GameRoom::new("ABCDE", false, opts);
    let mut conns = Vec::new();
    for i in 0..n {
        let (conn, fake) = FakeConn::new(i as u64);
        let id = format!("p{i}");
        room.core().join(&id, &format!("Joueur{i}"), conn).unwrap();
        conns.push((id, Arc::new(Mutex::new(fake))));
    }
    Setup { room, clock, conns }
}

fn team(core: &RoomCore, id: &str) -> Team {
    core.player(id).unwrap().lobby.team
}

fn lock(s: &Setup, id: &str, champion: &str) {
    s.handle(
        id,
        ClientMsg::DraftLock {
            champion_id: champion.into(),
        },
    );
}

#[test]
fn equilibre_les_equipes_et_exige_8_joueurs_prets() {
    let s = setup(8);
    let blue = s
        .room
        .core()
        .players
        .iter()
        .filter(|p| p.lobby.team == Team::Blue)
        .count();
    assert_eq!(blue, 4);
    s.handle("p0", ClientMsg::StartGame);
    assert_eq!(s.room.core().phase, RoomPhase::Lobby);
    assert!(s.conn("p0").last_error().is_some());
    s.ready_all();
    s.handle("p1", ClientMsg::StartGame); // pas hôte
    assert_eq!(s.room.core().phase, RoomPhase::Lobby);
    s.handle("p0", ClientMsg::StartGame);
    assert_eq!(s.room.core().phase, RoomPhase::Draft);
}

#[test]
fn ordre_1_2_2_2_1_et_picks_uniques() {
    let s = setup(8);
    s.ready_all();
    s.handle("p0", ClientMsg::StartGame);
    let turns = s.room.core().draft.as_ref().unwrap().turns.clone();
    let lens: Vec<usize> = turns.iter().map(Vec::len).collect();
    assert_eq!(lens, [1, 2, 2, 2, 1]);
    {
        let core = s.room.core();
        let teams: Vec<Team> = turns.iter().map(|t| team(&core, &t[0])).collect();
        assert_eq!(
            teams,
            [Team::Blue, Team::Red, Team::Blue, Team::Red, Team::Blue]
        );
    }
    let b1 = turns[0][0].clone();
    let (r1, r2) = (turns[1][0].clone(), turns[1][1].clone());
    let locked = |id: &str| {
        s.room
            .core()
            .draft
            .as_ref()
            .unwrap()
            .pick(id)
            .unwrap()
            .locked
    };
    let current = || s.room.core().draft.as_ref().unwrap().current_turn;
    // Hors tour : refusé.
    lock(&s, &r1, "nyx");
    assert!(!locked(&r1));
    lock(&s, &b1, "nyx");
    assert_eq!(current(), 1);
    // Champion déjà pris : refusé.
    lock(&s, &r1, "nyx");
    assert!(!locked(&r1));
    lock(&s, &r1, "sable");
    assert_eq!(current(), 1);
    lock(&s, &r2, "ilyra");
    assert_eq!(current(), 2);
}

#[test]
fn expiration_survol_ou_hasard_puis_finalisation_chargement_et_partie() {
    let s = setup(8);
    s.ready_all();
    s.handle("p0", ClientMsg::StartGame);
    let b1 = s.room.core().draft.as_ref().unwrap().turns[0][0].clone();
    s.handle(
        &b1,
        ClientMsg::DraftHover {
            champion_id: "zephyr".into(),
        },
    );
    s.advance(DRAFT_TURN_SECONDS + 0.1);
    let pick = |id: &str| {
        s.room
            .core()
            .draft
            .as_ref()
            .unwrap()
            .pick(id)
            .unwrap()
            .clone()
    };
    assert_eq!(pick(&b1).champion_id.as_deref(), Some("zephyr"));
    for _ in 0..4 {
        s.advance(DRAFT_TURN_SECONDS + 0.1);
    }
    {
        let core = s.room.core();
        let d = core.draft.as_ref().unwrap();
        assert!(d.finalize_ends_at.is_some());
        let mut ids: Vec<String> = d
            .picks
            .iter()
            .filter_map(|p| p.champion_id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 8);
    }
    s.handle(
        &b1,
        ClientMsg::DraftSummoners {
            summoners: [SummonerId::Flash, SummonerId::Smite],
        },
    );
    s.handle(
        &b1,
        ClientMsg::DraftSummoners {
            summoners: [SummonerId::Flash, SummonerId::Flash],
        },
    ); // invalide
    assert_eq!(pick(&b1).summoners, [SummonerId::Flash, SummonerId::Smite]);
    s.advance(FINALIZE_SECONDS + 0.1);
    assert_eq!(s.room.core().phase, RoomPhase::Loading);
    assert!(s.conn("p3").has_loading());
    s.advance(LOADING_SECONDS + 0.1);
    assert_eq!(s.room.core().phase, RoomPhase::Game);
    let start = {
        let mut c = s.conn(&b1);
        c.pump();
        c.msgs
            .iter()
            .rev()
            .find_map(|m| match m {
                ServerMsg::GameStart { start } => Some(start.clone()),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(start.players.len(), 8);
    let me = start.players.iter().find(|p| p.id == b1).unwrap();
    assert_eq!(me.champion_id, "zephyr");
    assert_eq!(me.summoners, [SummonerId::Flash, SummonerId::Smite]);
    let core = s.room.core();
    let c = core.game.as_ref().unwrap().world.champion_of(&b1).unwrap();
    let sums = &c.champ.as_ref().unwrap().summoners;
    assert_eq!((sums.d, sums.f), (SummonerId::Flash, SummonerId::Smite));
}

#[test]
fn les_survols_ne_sont_visibles_que_des_allies() {
    let s = setup(8);
    s.ready_all();
    s.handle("p0", ClientMsg::StartGame);
    let b1 = s.room.core().draft.as_ref().unwrap().turns[0][0].clone();
    s.handle(
        &b1,
        ClientMsg::DraftHover {
            champion_id: "kaelen".into(),
        },
    );
    let (blue, red) = {
        let core = s.room.core();
        let blue = core
            .players
            .iter()
            .find(|p| p.lobby.team == Team::Blue && p.lobby.id != b1)
            .unwrap()
            .lobby
            .id
            .clone();
        let red = core
            .players
            .iter()
            .find(|p| p.lobby.team == Team::Red)
            .unwrap()
            .lobby
            .id
            .clone();
        (blue, red)
    };
    assert_eq!(
        s.conn(&blue).last_draft_hover(&b1).as_deref(),
        Some("kaelen")
    );
    assert_eq!(s.conn(&red).last_draft_hover(&b1).as_deref(), Some(""));
}

// ------------------------------------------------------------------------------------------------
// Partie
// ------------------------------------------------------------------------------------------------

fn in_game() -> Setup {
    let s = setup(2);
    s.room.core().dev_mode = true;
    s.handle("p0", ClientMsg::StartGame);
    let turns = s.room.core().draft.as_ref().unwrap().turns.clone();
    for turn in turns {
        for id in turn {
            lock(&s, &id, if id == "p0" { "grondak" } else { "sable" });
        }
    }
    s.advance(FINALIZE_SECONDS + 0.1);
    s.advance(LOADING_SECONDS + 0.1);
    assert_eq!(s.room.core().phase, RoomPhase::Game);
    s
}

#[test]
fn envoie_un_snapshot_complet_puis_des_deltas() {
    let s = in_game();
    for _ in 0..6 {
        s.room.core().tick_game();
    }
    let snaps = s.conn("p0").snapshots();
    assert!(snaps.len() >= 3);
    assert!(snaps[0].removed.is_none());
    let me = s
        .room
        .core()
        .game
        .as_ref()
        .unwrap()
        .world
        .champion_id_of("p0");
    assert_eq!(snaps[0].self_state.as_ref().map(|s| s.entity_id), me);
    assert_eq!(snaps[1].removed, Some(vec![]));
    // Rien ne bouge : delta vide.
    assert!(snaps[2].entities.is_empty());
}

#[test]
fn reconnexion_game_start_renvoye_et_snapshot_complet() {
    let s = in_game();
    s.room.core().tick_game();
    s.room.core().leave("p1");
    assert!(!s.room.core().player("p1").unwrap().lobby.connected);
    let (conn, mut c2) = FakeConn::new(99);
    s.room.core().join("p1", "Joueur1", conn).unwrap();
    c2.pump();
    let you = c2.msgs.iter().rev().find_map(|m| match m {
        ServerMsg::GameStart { start } => Some(start.you.clone()),
        _ => None,
    });
    assert_eq!(you.as_deref(), Some("p1"));
    s.room.core().tick_game();
    s.room.core().tick_game();
    let snap = c2.snapshots().into_iter().next().unwrap();
    assert!(snap.removed.is_none());
    assert!(!snap.entities.is_empty());
}

#[test]
fn fin_de_partie_game_over_avec_stats_puis_retour_au_lobby() {
    let s = in_game();
    s.room.core().game.as_mut().unwrap().world.winner = Some(Team::Red);
    s.room.core().tick_game();
    assert_eq!(s.room.core().phase, RoomPhase::End);
    let over = {
        let mut c = s.conn("p0");
        c.pump();
        c.msgs
            .iter()
            .rev()
            .find_map(|m| match m {
                ServerMsg::GameOver { result } => Some(result.clone()),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(over.winner, Team::Red);
    assert_eq!(over.stats.len(), 2);
    assert_eq!((over.stats[0].kills, over.stats[0].deaths), (0, 0));
    s.handle("p1", ClientMsg::ReturnToLobby);
    assert_eq!(s.room.core().phase, RoomPhase::Lobby);
    assert!(s.room.core().game.is_none());
}

// Test ajouté : une salle réelle tourne seule (tâches tokio) du lobby à la partie.
#[tokio::test]
async fn une_salle_automatique_lance_la_partie_et_envoie_des_snapshots() {
    let opts = RoomOptions {
        timings: Some(DraftTimings {
            turn: 0.2,
            finalize: 0.2,
            loading: 0.2,
        }),
        seed: Some(3),
        skip_draft: false,
        ..RoomOptions::default()
    };
    let room = GameRoom::new("AUTO1", true, opts);
    let (conn, mut fake) = FakeConn::new(1);
    room.core().join("solo", "Solo", conn).unwrap();
    room.core().handle("solo", ClientMsg::StartGame);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while fake.snapshots().len() < 3 && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        fake.snapshots().len() >= 3,
        "la boucle de jeu doit envoyer des snapshots"
    );
    assert_eq!(room.core().phase, RoomPhase::Game);
}
