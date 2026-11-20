//! Démo des vues d'entités : `LOV_VIEWLAB=1` aligne, sans serveur, un exemplaire de chaque modèle
//! (sbires des deux équipes, monstres, drakes des six éléments, structures, balise, champion) et les fait
//! passer en boucle par les animations idle → run → attack → death. Remplace, côté client, les tests
//! visuels : on y vérifie silhouettes, couleurs et animations d'un coup d'œil.

use std::collections::HashMap;

use bevy::prelude::*;
use lov_shared::protocol::{Score, ServerMsg, Snapshot, TeamScore};
use lov_shared::types::{
    ChampSnapshot, DragonElement, EntityId, EntityKind, EntitySnapshot, TeamOrNeutral,
};

use super::entities::DragonElementState;
use super::map::MAP_SIZE;
use crate::net::{GameState, now_ms};

const TICK: f32 = 1.0 / 30.0;
/// Durée de chaque animation du cycle, en secondes.
const PHASE: f32 = 2.5;
const ANIMS: [&str; 4] = ["idle", "run", "attack", "death"];
/// Écart entre deux modèles (unités de jeu) : petits modèles, puis monstres épiques et structures.
const STEP: f32 = 300.0;
const BIG_STEP: f32 = 620.0;
/// Ordonnée de chaque rangée par rapport au centre, et celle du drake.
const ROW_Y: [f32; 5] = [600.0, 300.0, 0.0, -500.0, -1150.0];
const DRAKE_Y: f32 = -1750.0;
const ELEMENTS: [DragonElement; 6] = [
    DragonElement::Infernal,
    DragonElement::Ocean,
    DragonElement::Mountain,
    DragonElement::Cloud,
    DragonElement::Hextech,
    DragonElement::Chemtech,
];

pub fn enabled() -> bool {
    std::env::var_os("LOV_VIEWLAB").is_some()
}

/// Rangées de la démo : (genre, sous-type, équipe).
fn rows() -> Vec<Vec<(EntityKind, Option<&'static str>, TeamOrNeutral)>> {
    use EntityKind::*;
    use TeamOrNeutral::*;
    let minions = ["melee", "caster", "cannon", "super"];
    let mut team_rows = Vec::new();
    for team in [Blue, Red] {
        let mut row: Vec<_> = minions.iter().map(|m| (Minion, Some(*m), team)).collect();
        row.push((Champion, Some("kaelen"), team));
        row.push((Ward, None, team));
        team_rows.push(row);
    }
    let jungle = [
        "wolves",
        "wolf_small",
        "raptors",
        "raptor_small",
        "krugs",
        "krug_small",
        "gromp",
    ];
    let epic = [
        "blue_buff",
        "red_buff",
        "scuttle",
        "grub",
        "herald",
        "leviathan",
        "elder_dragon",
    ];
    vec![
        team_rows[0].clone(),
        team_rows[1].clone(),
        jungle
            .iter()
            .map(|m| (Monster, Some(*m), Neutral))
            .collect(),
        epic.iter().map(|m| (Monster, Some(*m), Neutral)).collect(),
        vec![
            (Tower, Some("outer"), Blue),
            (Inhibitor, None, Red),
            (Nexus, None, Blue),
            (Monster, Some("unknown"), Neutral),
        ],
    ]
}

#[derive(Resource, Default)]
struct Lab {
    entities: HashMap<EntityId, EntitySnapshot>,
    tick: u64,
    clock: f32,
    accumulator: f32,
    /// Nombre de drakes déjà montrés : un nouvel élément à chaque cycle.
    drakes: usize,
}

fn snapshot(
    id: EntityId,
    kind: EntityKind,
    sub: Option<&str>,
    team: TeamOrNeutral,
    pos: Vec2,
) -> EntitySnapshot {
    EntitySnapshot {
        id,
        kind,
        team,
        x: f64::from(pos.x),
        y: f64::from(pos.y),
        facing: -0.6,
        // PV variés d'un modèle à l'autre : barres, graduations et bouclier visibles
        hp: 1000.0 * (0.35 + 0.6 * ((id * 37 % 11) as f64 / 10.0)),
        max_hp: 1000.0,
        mana: (kind == EntityKind::Champion).then_some(120.0),
        max_mana: (kind == EntityKind::Champion).then_some(300.0),
        shield: (kind == EntityKind::Champion).then_some(150.0),
        dead: None,
        subtype: sub.map(str::to_owned),
        owner_id: None,
        level: (kind == EntityKind::Champion).then_some(id + 3),
        anim: None,
        buffs: None,
        champ: (kind == EntityKind::Champion).then(|| ChampSnapshot {
            player_id: format!("demo{id}"),
            name: "Joueur demo".into(),
            kills: 0,
            deaths: 0,
            assists: 0,
            cs: 0,
            items: vec![],
            respawn_in: None,
        }),
    }
}

const DRAKE_ID: EntityId = 999;

impl Lab {
    fn new() -> Self {
        let mut lab = Self::default();
        let center = Vec2::splat(MAP_SIZE / 2.0);
        let rows = rows();
        let mut id = 1;
        for (r, row) in rows.iter().enumerate() {
            for (c, (kind, sub, team)) in row.iter().enumerate() {
                let step = if r >= 3 { BIG_STEP } else { STEP };
                let x = (c as f32 - (row.len() - 1) as f32 / 2.0) * step;
                let y = ROW_Y[r];
                lab.entities.insert(
                    id,
                    snapshot(id, *kind, *sub, *team, center + Vec2::new(x, y)),
                );
                id += 1;
            }
        }
        let drake = center + Vec2::new(0.0, DRAKE_Y);
        lab.entities.insert(
            DRAKE_ID,
            snapshot(
                DRAKE_ID,
                EntityKind::Monster,
                Some("dragon"),
                TeamOrNeutral::Neutral,
                drake,
            ),
        );
        lab
    }

    /// Animation courante du cycle et indice du cycle.
    fn anim(&self) -> (&'static str, usize) {
        let k = (self.clock / PHASE) as usize;
        (ANIMS[k % ANIMS.len()], k / ANIMS.len())
    }

    fn publish(&mut self, state: &mut GameState) {
        self.tick += 1;
        let (anim, _) = self.anim();
        for e in self.entities.values_mut() {
            e.dead = (anim == "death").then_some(true);
            e.anim = Some(anim.to_owned());
        }
        let now = now_ms();
        let team = || TeamScore {
            kills: 0,
            towers: 0,
            dragons: vec![],
            grubs: 0,
        };
        let snap = Snapshot {
            tick: self.tick,
            time: self.tick as f64 * f64::from(TICK),
            server_time: now,
            entities: self.entities.values().cloned().collect(),
            removed: None,
            patches: None,
            events: vec![],
            self_state: None,
            objective_timers: None,
            score: Score {
                blue: team(),
                red: team(),
            },
        };
        state.route(
            ServerMsg::Snapshot {
                snap: Box::new(snap),
            },
            now,
        );
    }
}

pub struct ViewLabPlugin;

impl Plugin for ViewLabPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            |mut commands: Commands,
             mut state: ResMut<GameState>,
             mut rig: ResMut<super::camera::CameraRig>| {
                let mut lab = Lab::new();
                state.my_entity = Some(16); // le krug, au centre de la grille : la caméra se centre sur la démo
                state.status = "DEMO DES VUES (LOV_VIEWLAB)".into();
                rig.locked = true; // vue fixe : le défilement au bord de l'écran ne l'emporte pas
                lab.publish(&mut state);
                commands.insert_resource(lab);
            },
        )
        .add_systems(Update, run_lab);
    }
}

fn run_lab(
    mut lab: ResMut<Lab>,
    mut state: ResMut<GameState>,
    mut element: ResMut<DragonElementState>,
    time: Res<Time>,
) {
    lab.accumulator = (lab.accumulator + time.delta_secs()).min(3.0 * TICK);
    while lab.accumulator >= TICK {
        lab.accumulator -= TICK;
        lab.clock += TICK;
        // un nouvel élément de drake à chaque cycle : la vue périmée est reconstruite
        let (_, cycle) = lab.anim();
        if cycle >= lab.drakes {
            element.0 = Some(ELEMENTS[cycle % ELEMENTS.len()]);
            lab.drakes = cycle + 1;
        }
        lab.publish(&mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_exemplaire_de_chaque_modele() {
        let lab = Lab::new();
        let monsters = lab
            .entities
            .values()
            .filter(|e| e.kind == EntityKind::Monster)
            .count();
        // 7 de jungle, 7 épiques, l'inconnu et le drake
        assert_eq!(monsters, 16);
        assert!(lab.entities.values().all(|e| {
            (0.0..f64::from(MAP_SIZE)).contains(&e.x) && (0.0..f64::from(MAP_SIZE)).contains(&e.y)
        }));
    }

    #[test]
    fn cycle_des_animations() {
        let mut lab = Lab::new();
        assert_eq!(lab.anim(), ("idle", 0));
        lab.clock = PHASE * 3.5;
        assert_eq!(lab.anim(), ("death", 0));
        lab.clock = PHASE * 4.1;
        assert_eq!(lab.anim(), ("idle", 1));
    }
}
