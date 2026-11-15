//! Monstres neutres (camps de jungle et objectifs épiques) : port de
//! `packages/server/src/world-ai/monsters.ts`.
//!
//! Différences avec le TypeScript :
//! - `installMonsterHooks` était appelé paresseusement par `spawnMonster` et protégé par un `WeakSet`
//!   par monde. Ici les systèmes qui font apparaître des monstres l'appellent dans leur `init` ; le hook
//!   est une fonction pure, l'installer deux fois (jungle et objectifs) ne change rien ;
//! - les fonctions désignent les entités par `EntityId`, et `update_monster` reçoit `core_combat`
//!   (voir `runtime::attack`).

use lov_shared::data::monsters::{MONSTERS, MonsterDef};
use lov_shared::math::dist2;
use lov_shared::types::{EntityId, EntityKind, MonsterType, TeamOrNeutral, Vec2};

use crate::systems::leveling::XpGlobal;
use crate::world::{Entity, World};
use crate::world_ai::runtime::{PveCombat, Rewards, XpReward, attack, stats_with};

/// `entity.data.monster`.
#[derive(Debug, Clone, PartialEq)]
pub struct MonsterState {
    pub home: Vec2,
    pub leash: f64,
    pub last_combat: f64,
    pub resetting: bool,
    /// Camp de jungle d'origine (`blue_wolves`…).
    pub camp: Option<String>,
    /// Objectif épique (dragon, Héraut, Larves, Léviathan).
    pub epic: bool,
}

/// `extra` de `spawnMonster` : camp, position de retour (si différente du point d'apparition), épique.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MonsterExtra {
    pub camp: Option<String>,
    pub home: Option<Vec2>,
    pub epic: bool,
}

/// Définition d'un type de monstre.
pub fn monster_def(t: MonsterType) -> &'static MonsterDef {
    MONSTERS
        .iter()
        .find(|(m, _)| *m == t)
        .map(|(_, d)| d)
        .unwrap_or_else(|| unreachable!("MONSTERS couvre tous les types de monstres"))
}

/// `Math.round` de JavaScript (demis arrondis vers +infini).
fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// Un monstre qui revient à son camp n'attaque pas, et le Carapateur n'attaque jamais.
pub fn install_monster_hooks(world: &mut World) {
    world.hooks.can_attack.push(Box::new(|world, id| {
        world.get(id).is_none_or(|e| {
            e.data
                .get::<MonsterState>()
                .is_none_or(|s| !s.resetting && e.subtype.as_deref() != Some("scuttle"))
        })
    }));
}

/// Fait apparaître un monstre neutre ; ses PV et son AD grandissent de 8 % toutes les 5 minutes.
pub fn spawn_monster(
    world: &mut World,
    t: MonsterType,
    pos: Vec2,
    leash: f64,
    extra: MonsterExtra,
) -> EntityId {
    let def = monster_def(t);
    let growth = 1.0 + (world.time / 300.0).floor() * 0.08;
    let hp = js_round(def.hp * growth);
    let speed = if t == MonsterType::Scuttle {
        200.0
    } else {
        300.0
    };
    let radius = if extra.epic { 100.0 } else { 55.0 };
    let mut e = Entity::new(
        EntityKind::Monster,
        TeamOrNeutral::Neutral,
        pos,
        radius,
        hp,
        0.0,
        stats_with(hp, def.ad * growth, def.armor, def.range, speed, 0.75),
    );
    e.subtype = Some(t.as_str().to_owned());
    e.data.insert(PveCombat);
    e.data.insert(XpReward(def.xp));
    e.data.insert(XpGlobal(extra.epic));
    e.data.insert(MonsterState {
        home: extra.home.unwrap_or(pos),
        leash,
        last_combat: world.time,
        resetting: false,
        camp: extra.camp,
        epic: extra.epic,
    });
    e.data.insert(Rewards {
        gold: Some(def.gold),
        xp: Some(def.xp),
        team_gold: def.team_gold,
        smite_required: Some(!extra.epic),
        xp_radius: Some(1400.0),
        ..Rewards::default()
    });
    world.spawn(e)
}

/// Le monstre `target` prend `source` pour cible (sauf s'il revient à son camp).
pub fn aggro_monster(world: &mut World, target: EntityId, source: EntityId) {
    let time = world.time;
    let Some(src) = world.get(source) else { return };
    if src.dead || src.team == TeamOrNeutral::Neutral {
        return;
    }
    let Some(t) = world.get_mut(target) else {
        return;
    };
    if t.dead {
        return;
    }
    let Some(state) = t.data.get_mut::<MonsterState>() else {
        return;
    };
    if state.resetting {
        return;
    }
    state.last_combat = time;
    t.attack_target = Some(source);
}

/// Un tick d'IA : poursuite dans la laisse, retour au camp avec régénération, attaque. Renvoie vrai si
/// une attaque est partie (repli isolé, sans le combat central).
pub fn update_monster(world: &mut World, id: EntityId, dt: f64, core_combat: bool) -> bool {
    let time = world.time;
    let Some(e) = world.get(id) else { return false };
    if e.dead {
        return false;
    }
    let Some(state) = e.data.get::<MonsterState>() else {
        return false;
    };
    let (home, leash, last_combat) = (state.home, state.leash, state.last_combat);
    let chase = (leash + 250.0) * (leash + 250.0);
    let target = world
        .get_opt(e.attack_target)
        .filter(|t| !t.dead && dist2(t.pos, home) <= chase)
        .map(|t| t.id);
    let too_far = dist2(e.pos, home) > leash * leash;
    let start_reset =
        too_far || (target.is_none() && e.hp < e.stats.max_hp && time - last_combat >= 5.0);

    let Some(e) = world.get_mut(id) else {
        return false;
    };
    let Entity {
        data,
        hp,
        stats,
        pos,
        attack_target,
        move_target,
        ..
    } = e;
    let Some(state) = data.get_mut::<MonsterState>() else {
        return false;
    };
    if start_reset {
        state.resetting = true;
    }
    if state.resetting {
        *attack_target = None;
        *move_target = Some(home);
        *hp = stats.max_hp.min(*hp + stats.max_hp * dt * 0.25);
        if dist2(*pos, home) < 60.0 * 60.0 && *hp == stats.max_hp {
            state.resetting = false;
            *pos = home;
            *move_target = None;
        }
        return false;
    }
    let Some(target) = target else {
        *attack_target = None;
        *move_target = None;
        return false;
    };
    state.last_combat = time;
    attack(world, id, target, None, core_combat)
}
