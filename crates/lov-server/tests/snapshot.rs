//! Tests des snapshots : port de `packages/server/test/snapshot.test.ts` et `perf.test.ts`.

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::time::Instant;

use flate2::Compression;
use flate2::write::DeflateEncoder;
use lov_server::snapshot::{
    ClientView, REFRESH_TICKS, ScoreTracker, SnapshotFrame, build_snapshot,
};
use lov_shared::constants::{SNAPSHOT_RATE, TICK_DT, TICK_RATE};
use lov_shared::data::champions::{CHAMPIONS, get_champion};
use lov_shared::map::LANE_PATHS;
use lov_shared::protocol::{ServerMsg, Snapshot, apply_entity_patch, encode};
use lov_shared::types::{
    DamageType, DragonElement, EntityId, EntityKind, EntitySnapshot, GameEvent, Lane, MonsterType,
    Stats, StructureKind, SummonerId, Team, TeamOrNeutral, Vec2,
};
use lov_sim::combat::damage::{DamageOpts, ShieldOpts, apply_damage, apply_shield, remove_shield};
use lov_sim::systems::combat::{KeepCorpse, combat_system};
use lov_sim::systems::input::input_system;
use lov_sim::systems::movement::movement_system;
use lov_sim::systems::registry::create_systems;
use lov_sim::systems::vision::{SightRadius, vision_system};
use lov_sim::{CreateChampion, Entity, System, World, create_champion};

fn v(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

fn base_stats() -> Stats {
    Stats {
        max_hp: 1000.0,
        hp_regen: 0.0,
        max_mana: 0.0,
        mana_regen: 0.0,
        ad: 100.0,
        ap: 0.0,
        armor: 0.0,
        mr: 0.0,
        attack_speed: 1.0,
        attack_range: 150.0,
        move_speed: 300.0,
        crit_chance: 0.0,
        ability_haste: 0.0,
        lifesteal: 0.0,
        armor_pen: 0.0,
        magic_pen: 0.0,
    }
}

/// `coreWorld()` : input, mouvement, combat, vision.
struct Core {
    world: World,
    systems: Vec<System>,
    players: u32,
}

impl Core {
    fn new() -> Self {
        let mut systems = vec![
            input_system(),
            movement_system(),
            combat_system(),
            vision_system(),
        ];
        let mut world = World::new(1);
        world.init(&mut systems);
        Self {
            world,
            systems,
            players: 0,
        }
    }

    fn step(&mut self) {
        self.world.step(&mut self.systems);
    }

    fn champ(&mut self, team: Team, pos: Vec2) -> EntityId {
        self.players += 1;
        spawn_champion(
            &mut self.world,
            &format!("p{}", self.players),
            team,
            "grondak",
            pos,
        )
    }

    fn dummy(&mut self, team: TeamOrNeutral, pos: Vec2, kind: EntityKind) -> EntityId {
        let s = base_stats();
        self.world
            .spawn(Entity::new(kind, team, pos, 40.0, s.max_hp, 0.0, s))
    }

    fn player(&self, id: EntityId) -> String {
        self.world
            .get(id)
            .unwrap()
            .champ
            .as_ref()
            .unwrap()
            .player_id
            .clone()
    }

    fn e(&mut self, id: EntityId) -> &mut Entity {
        self.world.get_mut(id).unwrap()
    }
}

fn spawn_champion(
    world: &mut World,
    player: &str,
    team: Team,
    champion: &str,
    pos: Vec2,
) -> EntityId {
    let def = get_champion(champion).unwrap();
    create_champion(
        world,
        CreateChampion {
            player_id: player.into(),
            name: "T".into(),
            team,
            champion_id: champion.into(),
            summoners: [SummonerId::Flash, SummonerId::Ignite],
            pos,
        },
        &def.base_stats,
    )
}

fn true_damage(world: &mut World, source: EntityId, target: EntityId, amount: f64) {
    apply_damage(
        world,
        DamageOpts::new(Some(source), target, amount, DamageType::True),
    );
}

#[test]
fn ne_transmet_pas_les_ennemis_invisibles_mais_toujours_les_structures() {
    let mut w = Core::new();
    let me = w.champ(Team::Blue, v(6000.0, 3000.0));
    let near = w.champ(Team::Red, v(6000.0, 3500.0));
    let far = w.champ(Team::Red, v(9000.0, 9000.0));
    let tower = w.dummy(TeamOrNeutral::Red, v(10000.0, 10000.0), EntityKind::Tower);
    w.step();
    let snap = build_snapshot(&w.world, &w.player(me), Team::Blue, Vec::new(), None);
    let ids: Vec<EntityId> = snap.entities.iter().map(|e| e.id).collect();
    assert!(ids.contains(&me));
    assert!(ids.contains(&near));
    assert!(ids.contains(&tower));
    assert!(!ids.contains(&far));
}

#[test]
fn filtre_les_evenements_d_unites_invisibles() {
    let mut w = Core::new();
    let me = w.champ(Team::Blue, v(6000.0, 3000.0));
    let far = w.champ(Team::Red, v(9000.0, 9000.0));
    let far_minion = w.dummy(TeamOrNeutral::Blue, v(9100.0, 9000.0), EntityKind::Minion);
    // Ne donne pas la vision : seul le fait d'être la cible compte.
    w.e(far_minion).data.insert(SightRadius(0.0));
    w.step();
    w.world.events.drain();
    let neutral = w.dummy(
        TeamOrNeutral::Neutral,
        v(9000.0, 9100.0),
        EntityKind::Minion,
    );
    true_damage(&mut w.world, far, neutral, 10.0);
    w.world.emit(GameEvent::GoldGain {
        target: far,
        amount: 10.0,
        pos: None,
    });
    for who in [far, me] {
        let player_id = w.player(who);
        w.world.emit(GameEvent::Ping {
            player_id,
            pos: v(0.0, 0.0),
            kind: "generic".into(),
        });
    }
    true_damage(&mut w.world, far, far_minion, 10.0);
    let events = w.world.events.drain();
    let evs = build_snapshot(&w.world, &w.player(me), Team::Blue, events, None).events;
    let kinds: Vec<&str> = evs
        .iter()
        .map(|e| match e {
            GameEvent::Ping { .. } => "ping",
            GameEvent::Damage { .. } => "damage",
            _ => "autre",
        })
        .collect();
    assert_eq!(kinds, ["ping", "damage"]);
}

fn build(
    w: &Core,
    view: &mut ClientView,
    score: &ScoreTracker,
    me: EntityId,
    events: Vec<GameEvent>,
) -> Snapshot {
    let frame = SnapshotFrame::new(&w.world, events, score.score(&w.world), 0.0);
    view.build(&frame, &w.player(me), Team::Blue)
}

#[test]
fn delta_entites_modifiees_seulement_et_removed_hors_vision() {
    let mut w = Core::new();
    let me = w.champ(Team::Blue, v(6000.0, 3000.0));
    let foe = w.champ(Team::Red, v(6000.0, 3500.0));
    let score = ScoreTracker::new(&mut w.world);
    let mut view = ClientView::new();
    w.step();
    let s1 = build(&w, &mut view, &score, me, Vec::new());
    assert!(s1.removed.is_none());
    assert_eq!(s1.entities.len(), 2);
    w.e(foe).hp -= 100.0;
    w.step();
    let s2 = build(&w, &mut view, &score, me, Vec::new());
    assert!(s2.entities.is_empty());
    let patches = s2.patches.unwrap();
    assert_eq!(patches.len(), 1);
    assert_eq!(patches[0].id, foe);
    assert_eq!(patches[0].hp, Some(w.e(foe).hp.round()));
    assert_eq!(
        serde_json::to_value(&patches[0])
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        2,
        "seuls id et hp"
    );
    w.e(foe).pos = v(10000.0, 10000.0);
    w.step();
    let s3 = build(&w, &mut view, &score, me, Vec::new());
    assert_eq!(s3.removed, Some(vec![foe]));
    assert!(s3.entities.is_empty());
}

#[test]
fn patches_la_vue_reconstruite_egale_le_snapshot_complet() {
    let mut w = Core::new();
    let me = w.champ(Team::Blue, v(6000.0, 3000.0));
    let foe = w.champ(Team::Red, v(6000.0, 3500.0));
    let score = ScoreTracker::new(&mut w.world);
    let mut view = ClientView::new();
    let mut client: BTreeMap<EntityId, EntitySnapshot> = BTreeMap::new();
    type Mutation = Box<dyn Fn(&mut Core)>;
    let mutations: Vec<Mutation> = vec![
        // champ ajouté (shield)
        Box::new(move |w| {
            apply_shield(&mut w.world, ShieldOpts::new(Some(foe), foe, 50.0));
        }),
        // objet imbriqué modifié (champ)
        Box::new(move |w| w.e(foe).champ.as_mut().unwrap().kills = 2),
        // champ supprimé (shield)
        Box::new(move |w| remove_shield(w.e(foe), None)),
        Box::new(move |w| w.e(foe).pos = v(6100.0, 3400.0)),
        // dead + respawnIn
        Box::new(move |w| true_damage(&mut w.world, me, foe, 1e6)),
        Box::new(|_| {}),
    ];
    for m in mutations {
        m(&mut w);
        w.step();
        let events = w.world.events.drain();
        let frame = SnapshotFrame::new(&w.world, events, score.score(&w.world), 0.0);
        let s = view.build(&frame, &w.player(me), Team::Blue);
        for id in s.removed.iter().flatten() {
            client.remove(id);
        }
        for e in &s.entities {
            client.insert(e.id, e.clone());
        }
        for p in s.patches.iter().flatten() {
            let next = apply_entity_patch(&client[&p.id], p);
            client.insert(p.id, next);
        }
        let truth = ClientView::new().build(&frame, &w.player(me), Team::Blue);
        let truth: BTreeMap<EntityId, EntitySnapshot> =
            truth.entities.into_iter().map(|e| (e.id, e)).collect();
        assert_eq!(client, truth);
    }
}

#[test]
fn rafraichissement_etale_en_moins_de_refresh_ticks() {
    let mut w = Core::new();
    let me = w.champ(Team::Blue, v(6000.0, 3000.0));
    let foe = w.champ(Team::Red, v(6000.0, 3500.0));
    let score = ScoreTracker::new(&mut w.world);
    let mut view = ClientView::new();
    let mut seen_full = HashSet::new();
    for i in 0..REFRESH_TICKS {
        w.step();
        let s = build(&w, &mut view, &score, me, Vec::new());
        if i > 0 {
            seen_full.extend(s.entities.iter().map(|e| e.id));
        }
    }
    assert_eq!(seen_full, HashSet::from([me, foe]));
}

#[test]
fn score_kills_et_tours() {
    let mut w = Core::new();
    let a = w.champ(Team::Blue, v(6000.0, 3000.0));
    let tracker = ScoreTracker::new(&mut w.world);
    w.e(a).champ.as_mut().unwrap().kills = 3;
    for plate in [None, Some(true)] {
        w.world.emit(GameEvent::StructureDestroyed {
            target: 99,
            kind: StructureKind::Tower,
            team: Team::Red,
            plate,
        });
    }
    w.world.emit(GameEvent::ObjectiveTaken {
        monster: MonsterType::Dragon,
        team: Team::Blue,
        killer: None,
        element: Some(DragonElement::Ocean),
    });
    let blue = tracker.score(&w.world).blue;
    assert_eq!(blue.kills, 3);
    assert_eq!(blue.towers, 1);
    assert_eq!(blue.dragons, ["ocean"]);
    assert_eq!(blue.grubs, 0);
}

// ------------------------------------------------------------------------------------------------
// perf.test.ts
// ------------------------------------------------------------------------------------------------

fn deflate_len(raw: &str) -> usize {
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
    let _ = enc.write_all(raw.as_bytes());
    enc.finish().map_or(0, |b| b.len())
}

#[test]
fn tick_et_snapshots_raisonnables_avec_8_champions_et_60_sbires() {
    let mut systems = create_systems();
    let mut world = World::new(1);
    for (i, def) in CHAMPIONS.iter().cycle().take(8).enumerate() {
        let team = if i < 4 { Team::Blue } else { Team::Red };
        let pos = v(
            5800.0 + (i % 4) as f64 * 100.0,
            5800.0 + if i < 4 { -300.0 } else { 300.0 },
        );
        spawn_champion(&mut world, &format!("p{i}"), team, &def.id, pos);
    }
    let lanes = [Lane::Top, Lane::Mid, Lane::Bot];
    for i in 0..60usize {
        let team = if i % 2 == 1 {
            TeamOrNeutral::Blue
        } else {
            TeamOrNeutral::Red
        };
        let path = LANE_PATHS.get(lanes[i % 3]);
        let p = path[path.len() / 2];
        let mut s = base_stats();
        s.max_hp = 500.0;
        s.ad = 5.0;
        s.attack_range = if i % 2 == 1 { 110.0 } else { 500.0 };
        let pos = v(p.x + (i % 7) as f64 * 30.0, p.y + (i % 5) as f64 * 30.0);
        let mut m = Entity::new(EntityKind::Minion, team, pos, 40.0, 500.0, 0.0, s);
        m.data.insert(KeepCorpse);
        world.spawn(m);
    }
    world.init(&mut systems);
    // Tout le monde attaque l'ennemi le plus proche.
    let units: Vec<(EntityId, TeamOrNeutral, Vec2)> =
        world.iter().map(|e| (e.id, e.team, e.pos)).collect();
    for &(id, team, pos) in &units {
        let best = units
            .iter()
            .filter(|o| o.1 != team)
            .min_by(|a, b| {
                let da = (a.2.x - pos.x).hypot(a.2.y - pos.y);
                let db = (b.2.x - pos.x).hypot(b.2.y - pos.y);
                da.total_cmp(&db)
            })
            .map(|o| o.0);
        world.get_mut(id).unwrap().attack_target = best;
    }
    for _ in 0..30 {
        world.step_dt(&mut systems, TICK_DT); // chauffe
    }
    let ticks = 300;
    let t0 = Instant::now();
    for _ in 0..ticks {
        world.step_dt(&mut systems, TICK_DT);
    }
    let per_tick = t0.elapsed().as_secs_f64() * 1000.0 / f64::from(ticks);
    assert!(per_tick < 5.0, "tick {per_tick:.2} ms");

    let score = ScoreTracker::new(&mut world);
    let mut view = ClientView::new();
    let full = {
        let frame = SnapshotFrame::new(&world, Vec::new(), score.score(&world), 0.0);
        encode(&view.build(&frame, "p0", Team::Blue)).unwrap()
    };
    let (mut bytes, mut zbytes) = (0usize, 0usize);
    // Une seconde de snapshots à la cadence réelle.
    for _ in 0..SNAPSHOT_RATE {
        for _ in 0..TICK_RATE / SNAPSHOT_RATE {
            world.step_dt(&mut systems, TICK_DT);
        }
        let events = world.events.drain();
        let frame = SnapshotFrame::new(&world, events, score.score(&world), 0.0);
        let snap = view.build(&frame, "p0", Team::Blue);
        let raw = encode(&ServerMsg::Snapshot {
            snap: Box::new(snap),
        })
        .unwrap();
        bytes += raw.len();
        // ≈ permessage-deflate sans contexte partagé.
        zbytes += deflate_len(&raw);
    }
    println!(
        "[perf] tick {per_tick:.2} ms, snapshot complet {} o, delta ~{} o ({} Ko/s brut, {} Ko/s compressé)",
        full.len(),
        bytes / SNAPSHOT_RATE as usize,
        bytes / 1024,
        zbytes / 1024
    );
    // Pire cas (68 unités visibles en combat permanent) : < 30 Ko/s une fois compressé.
    assert!(zbytes < 30 * 1024, "{zbytes} o/s compressés");
}
