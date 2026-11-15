//! Effets des drakes et des âmes : port de `packages/server/src/world-ai/objective-effects.ts`.
//!
//! Effets indépendants du recalcul des stats : chimtech (amplification sous 50 % de PV, soins), nuage
//! (vitesse après l'ultime, vitesse hors combat), infernal et hextech (explosions), océan (soins),
//! montagne (bouclier hors combat).
//!
//! L'état des objectifs est partagé par [`Objectives`] ; aucun verrou n'est tenu pendant un appel qui
//! peut redéclencher les hooks (dégâts, soins, boucliers, buffs).

use lov_shared::math::dist2;
use lov_shared::types::{
    AbilitySlot, DamageType, DragonElement, EntityId, EntityKind, GameEvent, TeamOrNeutral,
};

use crate::combat::buffs::{SourceMatch, remove_buff};
use crate::combat::damage::{HealOpts, ShieldOpts, apply_heal, apply_shield, time_since_combat};
use crate::systems::objectives::Objectives;
use crate::world::{EventKind, World};
use crate::world_ai::runtime::{BuffOptions, DamageRequest, add_buff, apply_damage, team_of};

/// `entity.data.objectiveSoul` : recharges des effets d'âme d'un champion.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SoulRuntime {
    pub next_burst: f64,
    pub ocean_until: f64,
    pub next_shield: f64,
}

/// `entity.data.cloudOocStacks` : cumuls du bonus de vitesse hors combat des drakes des nuages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloudOocStacks(pub usize);

fn stacks(dragons: &[DragonElement], element: DragonElement) -> usize {
    dragons.iter().filter(|&&d| d == element).count()
}

/// Âme de l'équipe de l'entité (aucune pour le neutre).
fn soul_of(objectives: &Objectives, team: TeamOrNeutral) -> Option<DragonElement> {
    let team = team_of(team)?;
    objectives.lock().team(team).soul
}

/// Branche les hooks des âmes. Appelé par l'`init` des objectifs.
pub fn install_objective_effects(world: &mut World, objectives: &Objectives) {
    let obj = objectives.clone();
    world.hooks.before_damage.push(Box::new(move |world, ctx| {
        let below_half = |id: EntityId| {
            world
                .get(id)
                .filter(|e| e.champ.is_some())
                .filter(|e| e.hp < e.stats.max_hp * 0.5)
                .map(|e| e.team)
        };
        if let Some(team) = ctx.credit.and_then(below_half)
            && soul_of(&obj, team) == Some(DragonElement::Chemtech)
        {
            ctx.multiplier *= 1.11;
        }
        if let Some(team) = below_half(ctx.target)
            && soul_of(&obj, team) == Some(DragonElement::Chemtech)
            && ctx.damage_type != DamageType::True
        {
            ctx.multiplier *= 0.89;
        }
    }));

    let obj = objectives.clone();
    world.hooks.before_heal.push(Box::new(move |world, ctx| {
        let Some(team) = world
            .get_opt(ctx.source)
            .filter(|s| s.champ.is_some())
            .and_then(|s| team_of(s.team))
        else {
            return;
        };
        let n = stacks(&obj.lock().team(team).dragons, DragonElement::Chemtech);
        ctx.amount *= 1.0 + n as f64 * 0.06;
    }));

    let obj = objectives.clone();
    world.events.on(EventKind::AbilityCast, move |world, ev| {
        let GameEvent::AbilityCast { source, slot, .. } = ev else {
            return;
        };
        let Some(team) = world.get(*source).map(|s| s.team) else {
            return;
        };
        if *slot == AbilitySlot::R && soul_of(&obj, team) == Some(DragonElement::Cloud) {
            add_buff(
                world,
                *source,
                "soul_cloud_speed",
                BuffOptions {
                    params: vec![("moveSpeedPercent".into(), 0.5)],
                    ..BuffOptions::new(6.0)
                },
            );
        }
    });

    let obj = objectives.clone();
    world
        .hooks
        .after_damage
        .push(Box::new(move |world, ctx, dealt| {
            let Some(source) = ctx.credit.or(ctx.source) else {
                return;
            };
            let time = world.time;
            let (Some(src), Some(target)) = (world.get(source), world.get(ctx.target)) else {
                return;
            };
            if dealt <= 0.0
                || src.champ.is_none()
                || src.dead
                || src.team == TeamOrNeutral::Neutral
                || target.dead
                || target.team == src.team
                || target.kind != EntityKind::Champion
                || ctx
                    .ability
                    .as_deref()
                    .is_some_and(|a| a.starts_with("soul_"))
            {
                return;
            }
            let (src_team, ad, ap) = (src.team, src.stats.ad, src.stats.ap);
            let target_pos = target.pos;
            let Some(soul) = soul_of(&obj, src_team) else {
                return;
            };
            let Some(src) = world.get_mut(source) else {
                return;
            };
            let active = src.data.get_or_insert_with(SoulRuntime::default);
            match soul {
                DragonElement::Ocean => {
                    active.ocean_until = time + 3.0;
                    return;
                }
                _ if time < active.next_burst => return,
                DragonElement::Infernal => active.next_burst = time + 3.0,
                DragonElement::Hextech => active.next_burst = time + 8.0,
                _ => return,
            }
            let in_reach = |world: &World, r: f64| -> Vec<EntityId> {
                world
                    .query(Some(EntityKind::Champion))
                    .filter(|v| {
                        !v.dead
                            && v.hp > 0.0
                            && v.team != src_team
                            && dist2(v.pos, target_pos) <= r * r
                    })
                    .map(|v| v.id)
                    .collect()
            };
            match soul {
                DragonElement::Infernal => {
                    let amount = 80.0 + ad * 0.2 + ap * 0.12;
                    world.emit(GameEvent::AreaEffect {
                        source,
                        fx_key: "soul_infernal".into(),
                        pos: target_pos,
                        radius: 250.0,
                        duration: 0.4,
                    });
                    for victim in in_reach(world, 250.0) {
                        apply_damage(
                            world,
                            DamageRequest {
                                source,
                                target: victim,
                                amount,
                                damage_type: DamageType::Magic,
                                ability: Some("soul_infernal".into()),
                            },
                        );
                    }
                }
                DragonElement::Hextech => {
                    for victim in in_reach(world, 500.0).into_iter().take(4) {
                        add_buff(
                            world,
                            victim,
                            "soul_hextech_slow",
                            BuffOptions {
                                source: Some(source),
                                params: vec![("slow".into(), 0.4)],
                                ..BuffOptions::new(2.0)
                            },
                        );
                        apply_damage(
                            world,
                            DamageRequest {
                                source,
                                target: victim,
                                amount: 80.0,
                                damage_type: DamageType::True,
                                ability: Some("soul_hextech".into()),
                            },
                        );
                    }
                }
                _ => {}
            }
        }));
}

/// Effets continus : régénération océan, vitesse hors combat des nuages, âmes océan et montagne.
/// `ids` est un tampon réutilisé.
pub fn update_objective_effects(
    world: &mut World,
    objectives: &Objectives,
    dt: f64,
    ids: &mut Vec<EntityId>,
) {
    let time = world.time;
    world.ids_into(Some(EntityKind::Champion), ids);
    for &id in ids.iter() {
        let Some(e) = world.get(id) else { continue };
        let Some(team) = team_of(e.team).filter(|_| !e.dead) else {
            continue;
        };
        let (oceans, clouds, soul) = {
            let st = objectives.lock();
            let t = st.team(team);
            (
                stacks(&t.dragons, DragonElement::Ocean),
                stacks(&t.dragons, DragonElement::Cloud),
                t.soul,
            )
        };
        if oceans > 0 && e.hp < e.stats.max_hp {
            let amount = (e.stats.max_hp - e.hp) * 0.025 * oceans as f64 * dt / 5.0;
            apply_heal(
                world,
                HealOpts {
                    silent: true,
                    ..HealOpts::new(Some(id), id, amount)
                },
            );
        }
        let out_of_combat = clouds > 0 && time_since_combat(world, id) > 5.0;
        let Some(e) = world.get_mut(id) else { continue };
        let held = e.data.get::<CloudOocStacks>().map(|c| c.0);
        if out_of_combat && held != Some(clouds) {
            e.data.insert(CloudOocStacks(clouds));
            add_buff(
                world,
                id,
                "dragon_cloud_ooc",
                BuffOptions {
                    params: vec![("moveSpeedPercent".into(), 0.04 * clouds as f64)],
                    ..BuffOptions::new(1e9)
                },
            );
        } else if !out_of_combat && held.is_some_and(|n| n > 0) {
            e.data.remove::<CloudOocStacks>();
            remove_buff(world, id, "dragon_cloud_ooc", SourceMatch::Any);
        }
        let Some(soul) = soul else { continue };
        let since_combat = time_since_combat(world, id);
        let Some(e) = world.get_mut(id) else { continue };
        let (max_hp, ap, ad) = (e.stats.max_hp, e.stats.ap, e.stats.ad);
        let active = *e.data.get_or_insert_with(SoulRuntime::default);
        if soul == DragonElement::Ocean && active.ocean_until > time {
            apply_heal(
                world,
                HealOpts {
                    silent: true,
                    ..HealOpts::new(Some(id), id, (130.0 + ap * 0.26 + max_hp * 0.03) * dt / 3.0)
                },
            );
            if let Some(e) = world.get_mut(id) {
                e.mana = e.stats.max_mana.min(e.mana + 30.0 * dt / 3.0);
            }
        }
        if soul == DragonElement::Mountain && since_combat >= 5.0 && time >= active.next_shield {
            if let Some(e) = world.get_mut(id) {
                e.data.get_or_insert_with(SoulRuntime::default).next_shield = time + 5.0;
            }
            let amount = 180.0 + max_hp * 0.05 + ad * 0.16 + ap * 0.13;
            apply_shield(
                world,
                ShieldOpts {
                    id: Some("soul_mountain".into()),
                    ..ShieldOpts::new(Some(id), id, amount)
                },
            );
        }
    }
}
