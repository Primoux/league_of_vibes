//! Intégration salle + simulation : port de `packages/server/test/room.integration.test.ts`.
//!
//! Deux joueurs pilotés uniquement par des `ClientMsg`, à travers une vraie salle (`GameRoom`, draft,
//! chargement), la liste complète des systèmes (`create_systems`, le défaut des salles) et les
//! snapshots envoyés aux connexions.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use lov_server::room::{Clock, Conn, GameRoom, RoomOptions};
use lov_shared::protocol::{ClientMsg, GameOver, RoomPhase, ServerMsg, Snapshot, decode};
use lov_shared::types::{AbilitySlot, EntityId, GameEvent, Team, Vec2};
use lov_sim::combat::buffs::{BuffOptions, add_buff};
use lov_sim::{Entity, World};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

/// Horloge de test (ms), avancée à la main.
#[derive(Default)]
struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now(&self) -> f64 {
        self.0.load(Ordering::Relaxed) as f64
    }
}

/// Connexion factice : garde les messages reçus (`class Conn` du TS).
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

    fn snaps(&mut self) -> Vec<Snapshot> {
        self.pump();
        self.msgs
            .iter()
            .filter_map(|m| match m {
                ServerMsg::Snapshot { snap } => Some((**snap).clone()),
                _ => None,
            })
            .collect()
    }

    fn game_over(&mut self) -> Option<GameOver> {
        self.pump();
        self.msgs.iter().find_map(|m| match m {
            ServerMsg::GameOver { result } => Some(result.clone()),
            _ => None,
        })
    }
}

/// Ce que les tests lisent d'une entité (`Entity` n'est pas clonable).
#[derive(Debug, Clone, Copy)]
struct View {
    pos: Vec2,
    hp: f64,
    max_hp: f64,
    dead: bool,
    q_level: Option<u32>,
    respawn_in: Option<f64>,
}

impl View {
    fn of(e: &Entity) -> Self {
        Self {
            pos: e.pos,
            hp: e.hp,
            max_hp: e.stats.max_hp,
            dead: e.dead,
            q_level: e.champ.as_ref().map(|c| c.ability_levels.q),
            respawn_in: e.champ.as_ref().map(|c| c.respawn_in),
        }
    }
}

/// `midLane()` : Kaelen (bleu) contre Sable (rouge), placés au milieu de la voie mid.
struct MidLane {
    room: GameRoom,
    blue: FakeConn,
    red: FakeConn,
    b: EntityId,
    r: EntityId,
}

impl MidLane {
    fn new() -> Self {
        let clock = Arc::new(TestClock::default());
        let opts = RoomOptions {
            clock: clock.clone(),
            manual: true,
            seed: Some(0), // `random: () => 0`
            ..RoomOptions::default()
        };
        let room = GameRoom::new("MIDXX", true, opts);
        let (c1, blue) = FakeConn::new(1);
        let (c2, red) = FakeConn::new(2);
        room.core().join("blue", "Bleu", c1).expect("bleu rejoint");
        room.core().join("red", "Rouge", c2).expect("rouge rejoint");
        assert_eq!(
            room.core().player("blue").map(|p| p.lobby.team),
            Some(Team::Blue)
        );
        assert_eq!(
            room.core().player("red").map(|p| p.lobby.team),
            Some(Team::Red)
        );
        room.core().handle("blue", ClientMsg::StartGame);
        let lock = |id: &str, champion: &str| {
            room.core().handle(
                id,
                ClientMsg::DraftLock {
                    champion_id: champion.into(),
                },
            );
        };
        lock("blue", "kaelen");
        lock("red", "sable");
        for _ in 0..3 {
            clock.0.fetch_add(10_000, Ordering::Relaxed);
            room.core().update();
        }
        assert_eq!(room.core().phase, RoomPhase::Game);
        let (b, r) = {
            let core = room.core();
            let world = &core.game.as_ref().expect("partie").world;
            (
                world.champion_id_of("blue").expect("champion bleu"),
                world.champion_id_of("red").expect("champion rouge"),
            )
        };
        let mut s = Self {
            room,
            blue,
            red,
            b,
            r,
        };
        // Place les deux champions au milieu de la voie mid.
        s.with_world(|w| {
            w.get_mut(b).expect("bleu").pos = Vec2 {
                x: 5800.0,
                y: 5800.0,
            };
            w.get_mut(r).expect("rouge").pos = Vec2 {
                x: 6300.0,
                y: 6300.0,
            };
        });
        s
    }

    fn with_world<T>(&mut self, f: impl FnOnce(&mut World) -> T) -> T {
        let mut core = self.room.core();
        f(&mut core.game.as_mut().expect("partie en cours").world)
    }

    fn e(&mut self, id: EntityId) -> View {
        self.with_world(|w| View::of(w.get(id).expect("entité")))
    }

    fn send(&self, id: &str, msg: ClientMsg) {
        self.room.core().handle(id, msg);
    }

    /// `run(s)` : `round(s * 30)` ticks de la salle (simulation + snapshots).
    fn run(&self, seconds: f64) {
        for _ in 0..(seconds * 30.0).round() as u32 {
            self.room.core().tick_game();
        }
    }
}

#[test]
fn commandes_deplacement_auto_attaques_sort_et_snapshots_coherents() {
    let mut s = MidLane::new();
    let (b, r) = (s.b, s.r);
    s.send(
        "blue",
        ClientMsg::LevelAbility {
            slot: AbilitySlot::Q,
        },
    );
    s.run(0.1);
    assert_eq!(s.e(b).q_level, Some(1));
    s.send("red", ClientMsg::Attack { target_id: b });
    s.run(3.0);
    let eb = s.e(b);
    assert!(eb.hp < eb.max_hp, "Sable a poursuivi puis tiré");
    s.send("blue", ClientMsg::Attack { target_id: r }); // Kaelen (mêlée) engage
    s.run(1.5);
    let rp = s.e(r).pos;
    s.send(
        "blue",
        ClientMsg::Cast {
            slot: AbilitySlot::Q,
            target_pos: Some(rp),
            target_id: None,
        },
    );
    s.run(1.0);
    let er = s.e(r);
    assert!(er.hp < er.max_hp);
    let bs = s.blue.snaps();
    assert!(bs.iter().any(|sn| sn.events.iter().any(|e| matches!(
        e,
        GameEvent::Damage { ability: Some(a), .. } if a == "kaelen_q"
    ))));
    // Chaque joueur voit l'autre et a reçu les événements de combat.
    let rs = s.red.snaps();
    assert!(bs.len() > 70, "{} snapshots", bs.len());
    let events = || bs.iter().flat_map(|sn| sn.events.iter());
    assert!(events().any(|e| matches!(e, GameEvent::AutoAttack { .. })));
    assert!(events().any(|e| matches!(e, GameEvent::AbilityCast { .. })));
    assert!(events().any(|e| matches!(e, GameEvent::Damage { .. })));
    assert!(rs.iter().any(|sn| sn.entities.iter().any(|e| e.id == b)));
    let last = bs.last().and_then(|sn| sn.self_state.as_ref());
    assert_eq!(last.map(|me| me.ability_levels.q), Some(1));
    assert!(s.with_world(|w| w.tick) > 100);
}

#[test]
fn un_champion_etourdi_ne_bouge_pas_et_n_attaque_pas() {
    let mut s = MidLane::new();
    let (b, r) = (s.b, s.r);
    s.with_world(|w| {
        let mut opts = BuffOptions::new(1.0);
        opts.source = Some(r);
        assert!(add_buff(w, b, "stun", opts).is_some());
    });
    let start = s.e(b).pos;
    s.send(
        "blue",
        ClientMsg::Move {
            pos: Vec2 {
                x: 5000.0,
                y: 5000.0,
            },
        },
    );
    s.run(0.5);
    assert_eq!(s.e(b).pos, start);
    s.send("blue", ClientMsg::Attack { target_id: r });
    let hp = s.e(r).hp;
    s.run(0.4);
    assert_eq!(s.e(r).hp, hp);
}

#[test]
fn mort_kill_credite_respawn_puis_fin_de_partie() {
    let mut s = MidLane::new();
    let (b, r) = (s.b, s.r);
    s.with_world(|w| w.get_mut(r).expect("rouge").hp = 30.0);
    s.send("blue", ClientMsg::Attack { target_id: r });
    s.run(3.0);
    assert!(s.e(r).dead);
    let snaps = s.blue.snaps();
    assert_eq!(snaps.last().map(|sn| sn.score.blue.kills), Some(1));
    assert!(snaps.iter().any(|sn| sn.events.iter().any(|e| matches!(
        e,
        GameEvent::Death { target, killer: Some(k), .. } if *target == r && *k == b
    ))));
    let respawn = s.e(r).respawn_in.unwrap_or(0.0);
    assert!(respawn > 0.0);
    s.run(respawn + 0.5);
    assert!(!s.e(r).dead);
    s.with_world(|w| w.winner = Some(Team::Blue));
    s.run(0.1);
    assert_eq!(s.room.core().phase, RoomPhase::End);
    let over = s.blue.game_over().expect("gameOver reçu");
    let stats = |id: &str| over.stats.iter().find(|p| p.player_id == id).cloned();
    let blue = stats("blue").expect("stats bleu");
    assert_eq!((blue.kills, blue.champion_id.as_str()), (1, "kaelen"));
    let red = stats("red").expect("stats rouge");
    assert_eq!((red.deaths, red.champion_id.as_str()), (1, "sable"));
}
