//! Scène de charge : `LOV_STRESS=<n>` remplit `GameState` d'entités synthétiques, sans serveur, pour
//! mesurer les FPS sur une scène aussi chargée qu'une vraie partie (champion, `n` sbires, tours,
//! projectiles). Les entités sont mises à jour à 30 Hz comme les snapshots du serveur, donc la synchro
//! des vues et le lissage de `entities.rs` travaillent exactement comme en jeu.

use bevy::prelude::*;
use serde_json::{Value, json};

use super::map::MAP_SIZE;
use crate::net::{EntityData, GameState};

const TICK: f32 = 1.0 / 30.0;
const MINION_SPEED: f32 = 350.0;
const PROJECTILE_RATE: f32 = 2.5;
const LATERAL_SPREAD: f32 = 160.0;
const CHAMPION_ID: u64 = 1;
const TOWER_ID: u64 = 2;
const MINION_ID: u64 = 100;
const PROJECTILE_ID: u64 = 100_000;

/// PROTOTYPE : copie de `LANE_PATHS` de `packages/shared/src/map.ts`, du côté bleu vers le côté rouge.
/// À remplacer par `lov-shared` dès que la PR `contract:` est mergée.
const LANE_PATHS: [&[(f32, f32)]; 3] = [
    &[
        (1300.0, 2000.0),
        (1100.0, 9500.0),
        (1600.0, 10400.0),
        (2500.0, 10900.0),
        (10000.0, 10700.0),
    ],
    &[(2000.0, 2000.0), (10000.0, 10000.0)],
    &[
        (2000.0, 1300.0),
        (9500.0, 1100.0),
        (10400.0, 1600.0),
        (10900.0, 2500.0),
        (10700.0, 10000.0),
    ],
];

/// PROTOTYPE : copie de `BLUE_TOWERS` de `map.ts` ; les tours rouges sont leur miroir (`mirror`).
const BLUE_TOWERS: [(f32, f32); 11] = [
    (1100.0, 8000.0),
    (1200.0, 5600.0),
    (1100.0, 3500.0),
    (4900.0, 4900.0),
    (4000.0, 4000.0),
    (3100.0, 3100.0),
    (8000.0, 1100.0),
    (5600.0, 1200.0),
    (3500.0, 1100.0),
    (1900.0, 1300.0),
    (1300.0, 1900.0),
];

/// Nombre d'unités demandé par `LOV_STRESS`, s'il est défini et valide.
pub fn units_from_env() -> Option<usize> {
    std::env::var("LOV_STRESS").ok()?.trim().parse().ok()
}

/// Générateur pseudo-aléatoire déterministe (splitmix64).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Flottant dans `[0, 1)`.
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

struct Minion {
    lane: usize,
    /// Distance parcourue le long de la voie.
    s: f32,
    /// +1 vers le camp rouge, -1 vers le bleu : aller-retour.
    dir: f32,
    lateral: f32,
}

struct Projectile {
    from: usize,
    to: usize,
    progress: f32,
}

#[derive(Resource)]
pub struct StressScene {
    rng: Rng,
    minions: Vec<Minion>,
    projectiles: Vec<Projectile>,
    /// Position de chaque unité (champion, tours, sbires), indexée comme les projectiles la visent.
    positions: Vec<Vec2>,
    lane_lengths: [f32; 3],
    accumulator: f32,
}

/// Point à la distance `s` sur la voie, décalé latéralement, et direction du segment (radians).
fn along_lane(lane: usize, s: f32, lateral: f32) -> (Vec2, f32) {
    let pts = LANE_PATHS[lane];
    let mut left = s;
    for w in pts.windows(2) {
        let (a, b) = (Vec2::new(w[0].0, w[0].1), Vec2::new(w[1].0, w[1].1));
        let len = a.distance(b);
        if left <= len || w[1] == pts[pts.len() - 1] {
            let dir = (b - a) / len;
            let p = a + dir * left.min(len) + dir.perp() * lateral;
            return (p.clamp(Vec2::ZERO, Vec2::splat(MAP_SIZE)), dir.to_angle());
        }
        left -= len;
    }
    (Vec2::ZERO, 0.0)
}

fn lane_length(lane: usize) -> f32 {
    LANE_PATHS[lane]
        .windows(2)
        .map(|w| Vec2::new(w[0].0, w[0].1).distance(Vec2::new(w[1].0, w[1].1)))
        .sum()
}

fn entity(id: u64, kind: &str, team: &str, pos: Vec2) -> EntityData {
    match json!({ "id": id, "kind": kind, "team": team, "x": pos.x, "y": pos.y, "facing": 0.0 }) {
        Value::Object(map) => map,
        _ => EntityData::new(),
    }
}

fn set_pose(e: &mut EntityData, pos: Vec2, facing: f32) {
    // Mise à jour en place : pas de nouvelle clé, donc pas d'allocation de chaîne.
    for (key, value) in [("x", pos.x), ("y", pos.y), ("facing", facing)] {
        if let Some(v) = e.get_mut(key) {
            *v = Value::from(f64::from(value));
        }
    }
}

impl StressScene {
    /// Crée la scène et remplit `entities` : 1 champion, 22 tours, `units` sbires, `units / 4` projectiles.
    pub fn new(units: usize, entities: &mut std::collections::HashMap<u64, EntityData>) -> Self {
        let mut rng = Rng(0x0010_F0F7_1BE5);
        let lane_lengths = [lane_length(0), lane_length(1), lane_length(2)];
        let mut positions = vec![Vec2::splat(MAP_SIZE / 2.0)];
        entities.insert(
            CHAMPION_ID,
            entity(CHAMPION_ID, "champion", "blue", positions[0]),
        );
        for (i, (x, y)) in BLUE_TOWERS.iter().enumerate() {
            let blue = Vec2::new(*x, *y);
            let red = Vec2::splat(MAP_SIZE) - blue;
            for (k, (team, pos)) in [("blue", blue), ("red", red)].into_iter().enumerate() {
                let id = TOWER_ID + (2 * i + k) as u64;
                entities.insert(id, entity(id, "tower", team, pos));
                positions.push(pos);
            }
        }
        let mut minions = Vec::with_capacity(units);
        for i in 0..units {
            let lane = i % 3;
            let blue = (i / 3) % 2 == 0;
            let id = MINION_ID + i as u64;
            entities.insert(
                id,
                entity(id, "minion", if blue { "blue" } else { "red" }, Vec2::ZERO),
            );
            minions.push(Minion {
                lane,
                s: rng.unit() * lane_lengths[lane],
                dir: if blue { 1.0 } else { -1.0 },
                lateral: (rng.unit() - 0.5) * 2.0 * LATERAL_SPREAD,
            });
            positions.push(Vec2::ZERO);
        }
        let mut projectiles = Vec::with_capacity(units / 4);
        for i in 0..units / 4 {
            let id = PROJECTILE_ID + i as u64;
            entities.insert(id, entity(id, "projectile", "neutral", Vec2::ZERO));
            projectiles.push(Projectile {
                from: rng.below(positions.len()),
                to: rng.below(positions.len()),
                progress: rng.unit(),
            });
        }
        let mut scene = Self {
            rng,
            minions,
            projectiles,
            positions,
            lane_lengths,
            accumulator: 0.0,
        };
        scene.tick(TICK, entities);
        scene
    }

    /// Avance la simulation de `dt` secondes et écrit les poses dans `entities`.
    fn tick(&mut self, dt: f32, entities: &mut std::collections::HashMap<u64, EntityData>) {
        let first_minion = 1 + 2 * BLUE_TOWERS.len();
        for (i, m) in self.minions.iter_mut().enumerate() {
            m.s += m.dir * MINION_SPEED * dt;
            let len = self.lane_lengths[m.lane];
            if m.s > len || m.s < 0.0 {
                m.s = m.s.clamp(0.0, len);
                m.dir = -m.dir;
            }
            let (pos, angle) = along_lane(m.lane, m.s, m.lateral);
            self.positions[first_minion + i] = pos;
            if let Some(e) = entities.get_mut(&(MINION_ID + i as u64)) {
                set_pose(
                    e,
                    pos,
                    if m.dir > 0.0 {
                        angle
                    } else {
                        angle + std::f32::consts::PI
                    },
                );
            }
        }
        let count = self.positions.len();
        for (i, p) in self.projectiles.iter_mut().enumerate() {
            p.progress += PROJECTILE_RATE * dt;
            if p.progress >= 1.0 {
                p.progress = 0.0;
                p.from = self.rng.below(count);
                p.to = self.rng.below(count);
            }
            let (a, b) = (self.positions[p.from], self.positions[p.to]);
            if let Some(e) = entities.get_mut(&(PROJECTILE_ID + i as u64)) {
                set_pose(e, a.lerp(b, p.progress), (b - a).to_angle());
            }
        }
    }
}

pub struct StressPlugin(pub usize);

impl Plugin for StressPlugin {
    fn build(&self, app: &mut App) {
        let units = self.0;
        app.add_systems(
            Startup,
            move |mut commands: Commands, mut state: ResMut<GameState>| {
                let scene = StressScene::new(units, &mut state.entities);
                state.my_entity = Some(CHAMPION_ID);
                state.status = format!("SCENE DE CHARGE, sans serveur ({units} sbires)");
                commands.insert_resource(scene);
            },
        )
        .add_systems(Update, run_scene);
    }
}

fn run_scene(mut scene: ResMut<StressScene>, mut state: ResMut<GameState>, time: Res<Time>) {
    // Au plus 3 ticks par image : une machine lente ralentit la scène au lieu de s'effondrer.
    scene.accumulator = (scene.accumulator + time.delta_secs()).min(3.0 * TICK);
    while scene.accumulator >= TICK {
        scene.accumulator -= TICK;
        scene.tick(TICK, &mut state.entities);
        state.snapshots += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{num, text};
    use std::collections::HashMap;

    fn count(entities: &HashMap<u64, EntityData>, kind: &str) -> usize {
        entities
            .values()
            .filter(|e| text(e, "kind") == kind)
            .count()
    }

    #[test]
    fn produit_le_nombre_demande() {
        let mut entities = HashMap::new();
        StressScene::new(300, &mut entities);
        assert_eq!(count(&entities, "minion"), 300);
        assert_eq!(count(&entities, "tower"), 22);
        assert_eq!(count(&entities, "champion"), 1);
        assert_eq!(count(&entities, "projectile"), 75);
    }

    #[test]
    fn tout_reste_dans_la_carte_et_bouge() {
        let mut entities = HashMap::new();
        let mut scene = StressScene::new(300, &mut entities);
        let before: Vec<f32> = (0..300)
            .map(|i| num(&entities[&(MINION_ID + i)], "x"))
            .collect();
        for _ in 0..3000 {
            scene.tick(TICK, &mut entities);
            for e in entities.values() {
                let (x, y) = (num(e, "x"), num(e, "y"));
                assert!((0.0..=MAP_SIZE).contains(&x) && (0.0..=MAP_SIZE).contains(&y));
            }
        }
        let moved = (0..300)
            .filter(|i| num(&entities[&(MINION_ID + i)], "x") != before[*i as usize])
            .count();
        assert!(moved > 250);
    }
}
