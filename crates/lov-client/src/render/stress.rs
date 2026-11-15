//! Scène de charge : `LOV_STRESS=<n>` remplit `GameState` d'entités synthétiques, sans serveur, pour
//! mesurer les FPS sur une scène aussi chargée qu'une vraie partie (champion, `n` sbires, tours,
//! projectiles). Les entités sont mises à jour à 30 Hz comme les snapshots du serveur, donc la synchro
//! des vues et le lissage de `entities.rs` travaillent exactement comme en jeu.

use std::collections::HashMap;

use bevy::prelude::*;
use lov_shared::map::{LANE_PATHS, TOWERS};
use lov_shared::types::{EntityId, EntityKind, EntitySnapshot, Team, TeamOrNeutral};

use super::from_shared;
use super::map::MAP_SIZE;
use crate::net::GameState;

const TICK: f32 = 1.0 / 30.0;
const MINION_SPEED: f32 = 350.0;
const PROJECTILE_RATE: f32 = 2.5;
const LATERAL_SPREAD: f32 = 160.0;
const CHAMPION_ID: EntityId = 1;
const TOWER_ID: EntityId = 2;
const MINION_ID: EntityId = 100;
const PROJECTILE_ID: EntityId = 100_000;

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
    /// Chemins des trois voies (haut, milieu, bas), du côté bleu vers le côté rouge.
    lanes: [Vec<Vec2>; 3],
    lane_lengths: [f32; 3],
    accumulator: f32,
}

/// Point à la distance `s` sur la voie, décalé latéralement, et direction du segment (radians).
fn along_lane(pts: &[Vec2], s: f32, lateral: f32) -> (Vec2, f32) {
    let mut left = s;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
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

fn lane_length(pts: &[Vec2]) -> f32 {
    pts.windows(2).map(|w| w[0].distance(w[1])).sum()
}

fn entity(id: EntityId, kind: EntityKind, team: TeamOrNeutral, pos: Vec2) -> EntitySnapshot {
    EntitySnapshot {
        id,
        kind,
        team,
        x: f64::from(pos.x),
        y: f64::from(pos.y),
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
        anim: None,
        buffs: None,
        champ: None,
    }
}

fn set_pose(e: &mut EntitySnapshot, pos: Vec2, facing: f32) {
    e.x = f64::from(pos.x);
    e.y = f64::from(pos.y);
    e.facing = f64::from(facing);
}

impl StressScene {
    /// Crée la scène et remplit `entities` : 1 champion, 22 tours, `units` sbires, `units / 4` projectiles.
    pub fn new(units: usize, entities: &mut HashMap<EntityId, EntitySnapshot>) -> Self {
        let mut rng = Rng(0x0010_F0F7_1BE5);
        let lanes = [&LANE_PATHS.top, &LANE_PATHS.mid, &LANE_PATHS.bot]
            .map(|path| path.iter().copied().map(from_shared).collect::<Vec<_>>());
        let lane_lengths = [
            lane_length(&lanes[0]),
            lane_length(&lanes[1]),
            lane_length(&lanes[2]),
        ];
        let mut positions = vec![Vec2::splat(MAP_SIZE / 2.0)];
        entities.insert(
            CHAMPION_ID,
            entity(
                CHAMPION_ID,
                EntityKind::Champion,
                TeamOrNeutral::Blue,
                positions[0],
            ),
        );
        for (i, tower) in TOWERS.iter().enumerate() {
            let pos = from_shared(tower.pos);
            let team = match tower.team {
                Team::Blue => TeamOrNeutral::Blue,
                Team::Red => TeamOrNeutral::Red,
            };
            let id = TOWER_ID + i as EntityId;
            entities.insert(id, entity(id, EntityKind::Tower, team, pos));
            positions.push(pos);
        }
        let mut minions = Vec::with_capacity(units);
        for i in 0..units {
            let lane = i % 3;
            let blue = (i / 3) % 2 == 0;
            let id = MINION_ID + i as EntityId;
            entities.insert(
                id,
                entity(
                    id,
                    EntityKind::Minion,
                    if blue {
                        TeamOrNeutral::Blue
                    } else {
                        TeamOrNeutral::Red
                    },
                    Vec2::ZERO,
                ),
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
            let id = PROJECTILE_ID + i as EntityId;
            entities.insert(
                id,
                entity(
                    id,
                    EntityKind::Projectile,
                    TeamOrNeutral::Neutral,
                    Vec2::ZERO,
                ),
            );
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
            lanes,
            lane_lengths,
            accumulator: 0.0,
        };
        scene.tick(TICK, entities);
        scene
    }

    /// Avance la simulation de `dt` secondes et écrit les poses dans `entities`.
    fn tick(&mut self, dt: f32, entities: &mut HashMap<EntityId, EntitySnapshot>) {
        let first_minion = 1 + TOWERS.len();
        for (i, m) in self.minions.iter_mut().enumerate() {
            m.s += m.dir * MINION_SPEED * dt;
            let len = self.lane_lengths[m.lane];
            if m.s > len || m.s < 0.0 {
                m.s = m.s.clamp(0.0, len);
                m.dir = -m.dir;
            }
            let (pos, angle) = along_lane(&self.lanes[m.lane], m.s, m.lateral);
            self.positions[first_minion + i] = pos;
            if let Some(e) = entities.get_mut(&(MINION_ID + i as EntityId)) {
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
            if let Some(e) = entities.get_mut(&(PROJECTILE_ID + i as EntityId)) {
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

    fn count(entities: &HashMap<EntityId, EntitySnapshot>, kind: EntityKind) -> usize {
        entities.values().filter(|e| e.kind == kind).count()
    }

    #[test]
    fn produit_le_nombre_demande() {
        let mut entities = HashMap::new();
        StressScene::new(300, &mut entities);
        assert_eq!(count(&entities, EntityKind::Minion), 300);
        assert_eq!(count(&entities, EntityKind::Tower), 22);
        assert_eq!(count(&entities, EntityKind::Champion), 1);
        assert_eq!(count(&entities, EntityKind::Projectile), 75);
    }

    #[test]
    fn tout_reste_dans_la_carte_et_bouge() {
        let mut entities = HashMap::new();
        let mut scene = StressScene::new(300, &mut entities);
        let before: Vec<f64> = (0..300).map(|i| entities[&(MINION_ID + i)].x).collect();
        let map_size = f64::from(MAP_SIZE);
        for _ in 0..3000 {
            scene.tick(TICK, &mut entities);
            for e in entities.values() {
                assert!((0.0..=map_size).contains(&e.x) && (0.0..=map_size).contains(&e.y));
            }
        }
        let moved = (0..300)
            .filter(|i| entities[&(MINION_ID + i)].x != before[*i as usize])
            .count();
        assert!(moved > 250);
    }
}
