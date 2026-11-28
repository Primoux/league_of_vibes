//! Passifs et actifs des objets : port de `packages/server/src/items/handlers.ts`.
//!
//! Les passifs s'enregistrent sur `world.hooks` / le bus d'événements ([`register_item_hooks`]) ; les
//! actifs sont déclenchés par la commande `useItem` ([`use_item`]). Les dégâts, soins et boucliers
//! passent exclusivement par `combat::damage`.
//!
//! Différences avec le TypeScript :
//! - les fonctions désignent l'entité par son `EntityId` (le monde est emprunté mutablement) ;
//! - `data.stasis` est le marqueur [`Stasis`] de `combat::buffs`, `data.dash` est [`DashData`] ;
//! - l’œil du Héraut (`data.heraldEye`) est le type [`HeraldEye`] de `items::state`.

use std::sync::Once;

use lov_shared::constants::MAP_SIZE;
use lov_shared::data::champions::get_champion;
use lov_shared::data::items::item;
use lov_shared::math::{clamp, dist};
use lov_shared::types::{
    AbilitySlot, DamageType, EntityId, EntityKind, GameEvent, ParamValue, PartialStats, Team,
    TeamOrNeutral, Vec2,
};

use crate::combat::buffs::{Stasis, StatModifiers, merge_stats, register_stat_source};
use crate::combat::damage::{
    DamageOpts, HealOpts, ShieldOpts, apply_damage, apply_heal, apply_shield, credit_of,
};
use crate::items::state::{
    ColossusCall, EconomyState, HealOverTime, HeraldEye, TRINKET_SLOT, econ, econ_ref,
    inventory_index,
};
use crate::nav::grid::nav_grid;
use crate::systems::movement::DashData;
use crate::world::{Entity, EventKind, World};

/// Ids des objets possédés (6 emplacements, puis le trinket).
pub fn owned_items(e: &Entity) -> impl Iterator<Item = &str> {
    let slots = e
        .champ
        .iter()
        .flat_map(|c| c.items.iter())
        .filter_map(|s| s.as_deref());
    let trinket = econ_ref(e).and_then(|st| st.trinket.as_deref());
    slots.chain(trinket)
}

pub fn has_passive(e: &Entity, passive_id: &str) -> bool {
    owned_items(e).any(|id| {
        item(id)
            .and_then(|d| d.passive.as_ref())
            .is_some_and(|p| p.id == passive_id)
    })
}

pub fn has_item(e: &Entity, item_id: &str) -> bool {
    owned_items(e).any(|id| id == item_id)
}

/// `has_passive` pour un id d'entité (faux si l'entité n'existe pas ou n'est pas un champion).
fn champ_has_passive(world: &World, id: Option<EntityId>, passive_id: &str) -> bool {
    world
        .get_opt(id)
        .is_some_and(|e| e.champ.is_some() && has_passive(e, passive_id))
}

/// Vrai si `ability` est l'un des quatre sorts du champion `e` (Q, W, E, R).
fn is_own_ability(e: &Entity, ability: Option<&str>) -> bool {
    let (Some(ability), Some(c)) = (ability, &e.champ) else {
        return false;
    };
    get_champion(&c.champion_id).is_some_and(|d| {
        [
            AbilitySlot::Q,
            AbilitySlot::W,
            AbilitySlot::E,
            AbilitySlot::R,
        ]
        .into_iter()
        .any(|slot| d.abilities.get(slot).id == ability)
    })
}

/// Somme des stats plates des objets possédés.
pub fn item_stats_of(e: &Entity) -> PartialStats {
    let mut out = PartialStats::default();
    for id in owned_items(e) {
        if let Some(def) = item(id) {
            merge_stats(&mut out, &def.stats, 1.0);
        }
    }
    out
}

static STAT_SOURCE: Once = Once::new();

/// Source de stats de l'actif « rafale » (+40 % de vitesse). Le registre des sources est global au
/// processus, comme en TypeScript : la source n'est enregistrée qu'une fois.
pub fn register_item_stat_source() {
    STAT_SOURCE.call_once(|| {
        // La poignée est volontairement oubliée : la source vit autant que le processus.
        let _handle = register_stat_source(|world, e| {
            let st = econ_ref(e)?;
            if st.rafale_until <= world.time {
                return None;
            }
            Some(StatModifiers {
                percent: PartialStats {
                    move_speed: Some(0.4),
                    ..PartialStats::default()
                },
                ..StatModifiers::default()
            })
        });
    });
}

/// Soigne (ou retire) le delta de PV max quand les objets changent (après achat, vente, annulation).
/// Le plafonnement aux PV max est fait au prochain recalcul des stats.
pub fn heal_item_max_hp_delta(e: &mut Entity, before: &PartialStats) {
    if e.dead {
        return;
    }
    let d = item_stats_of(e).max_hp.unwrap_or(0.0) - before.max_hp.unwrap_or(0.0);
    if d != 0.0 {
        e.hp += d;
    }
}

/// Enregistre les passifs d'objets. Appelé par l'`init` du système économie.
pub fn register_item_hooks(world: &mut World) {
    // Dégâts bonus contre les monstres (machette) et exécution (griffe de l'ombre).
    world.hooks.before_damage.push(Box::new(|world, ctx| {
        let Some(credit) = world.get_opt(ctx.credit).filter(|c| c.champ.is_some()) else {
            return;
        };
        let Some(target) = world.get(ctx.target) else {
            return;
        };
        // Machette : sur les auto-attaques et les sorts du champion, pas sur chaque instance de dégâts
        // (brûlures, renvois, objets…) (#316).
        if has_passive(credit, "chasseur")
            && target.kind == EntityKind::Monster
            && (ctx.is_auto_attack || is_own_ability(credit, ctx.ability.as_deref()))
        {
            ctx.amount += 15.0;
        }
        if has_passive(credit, "ombre")
            && target.stats.max_hp > 0.0
            && target.hp / target.stats.max_hp < 0.4
        {
            ctx.multiplier *= 1.1;
        }
    }));

    // Blessure grave (hache du bourreau) : réduit les soins reçus par la cible.
    // Vengeance (carapace du roc) : renvoie 10 % des dégâts subis à l'attaquant.
    world.hooks.after_damage.push(Box::new(|world, ctx, dealt| {
        let time = world.time;
        let credit = world
            .get_opt(ctx.credit)
            .map(|c| (c.id, c.team, c.dead, c.champ.is_some()));
        let Some(target_team) = world.get(ctx.target).map(|t| t.team) else {
            return;
        };
        if let Some((cid, cteam, _, true)) = credit
            && cteam != target_team
            && dealt > 0.0
            && champ_has_passive(world, Some(cid), "grievous")
            && let Some(t) = world.get_mut(ctx.target)
        {
            econ(t).grievous_until = time + 3.0;
        }
        // Un renvoi est marqué (`ability`) : on ne renvoie jamais un renvoi, sinon deux porteurs de
        // carapace se renverraient les dégâts à l'infini.
        let Some((cid, cteam, cdead, _)) = credit else {
            return;
        };
        let reflects = world
            .get(ctx.target)
            .is_some_and(|t| has_passive(t, "vengeance"));
        if reflects
            && cteam != target_team
            && !cdead
            && dealt > 0.0
            && ctx.damage_type != DamageType::True
            && ctx.ability.as_deref() != Some("item_vengeance")
        {
            apply_damage(
                world,
                DamageOpts {
                    silent: true,
                    ability: Some("item_vengeance".into()),
                    ..DamageOpts::new(Some(ctx.target), cid, dealt * 0.1, DamageType::Magic)
                },
            );
        }
    }));

    // Soins : anti-soin (blessure grave) et amplification (bénédiction de l'encensoir).
    world.hooks.before_heal.push(Box::new(|world, ctx| {
        if world
            .get(ctx.target)
            .and_then(econ_ref)
            .is_some_and(|st| st.grievous_until > world.time)
        {
            ctx.amount *= 0.6;
        }
        if champ_has_passive(world, ctx.source, "benediction") {
            ctx.amount *= 1.2;
        }
    }));

    // Auto-attaques : balises (1 dégât par attaque), sort-lame, étincelle.
    world.hooks.before_attack_hit.push(Box::new(|world, ctx| {
        if world
            .get(ctx.target)
            .is_some_and(|t| t.kind == EntityKind::Ward)
        {
            ctx.amount = 1.0;
            ctx.crit = false;
            ctx.crit_multiplier = 1.0;
            ctx.bonus = 0.0;
            return;
        }
        let Some(a) = world.get(ctx.attacker).filter(|a| a.champ.is_some()) else {
            return;
        };
        let spellblade =
            econ_ref(a).is_some_and(|st| st.spellblade_ready) && has_passive(a, "spellblade");
        if spellblade {
            ctx.bonus += a.stats.ap * 0.8;
            ctx.damage_type = DamageType::Magic;
            if let Some(a) = world.get_mut(ctx.attacker) {
                econ(a).spellblade_ready = false;
            }
        } else if has_passive(a, "etincelle") {
            ctx.bonus += 20.0;
        }
    }));

    // Rafale : source de stats. La stase (`Stasis`) est gérée nativement par `combat::buffs`.
    register_item_stat_source();

    // Un sort lancé arme le sort-lame.
    world.events.on(EventKind::AbilityCast, |world, ev| {
        let GameEvent::AbilityCast { source, .. } = ev else {
            return;
        };
        if champ_has_passive(world, Some(*source), "spellblade")
            && let Some(s) = world.get_mut(*source)
        {
            econ(s).spellblade_ready = true;
        }
    });

    // Passifs au last hit de sbire et à la mort d'un monstre.
    world.events.on(EventKind::Death, |world, ev| {
        let GameEvent::Death { target, killer, .. } = ev else {
            return;
        };
        let Some(credit) = credit_of(world, killer.filter(|&k| world.contains(k))) else {
            return;
        };
        let Some(t_kind) = world.get(*target).map(|t| t.kind) else {
            return;
        };
        if t_kind == EntityKind::Minion && champ_has_passive(world, Some(credit), "last_hit_heal") {
            apply_heal(world, HealOpts::new(Some(credit), credit, 4.0));
        }
        if t_kind == EntityKind::Monster && champ_has_passive(world, Some(credit), "chasseur") {
            apply_heal(world, HealOpts::new(Some(credit), credit, 10.0));
        }
    });
}

/// Effets périodiques des objets (potion, voile de la nuit, fin de stase). Appelé à chaque tick par
/// le système économie ; `ids` est un tampon réutilisé.
pub fn tick_items(world: &mut World, dt: f64, ids: &mut Vec<EntityId>) {
    world.ids_into(Some(EntityKind::Champion), ids);
    for &id in ids.iter() {
        let time = world.time;
        let Some(c) = world.get_mut(id) else { continue };
        if let Some(h) = econ(c).hot {
            apply_heal(
                world,
                HealOpts {
                    silent: true,
                    ..HealOpts::new(Some(id), id, h.total * dt / h.duration)
                },
            );
            if let Some(c) = world.get_mut(id) {
                let st = econ(c);
                if let Some(hot) = st.hot.as_mut() {
                    hot.remaining -= dt;
                    if hot.remaining <= 0.0 {
                        st.hot = None;
                    }
                }
            }
        }
        let voile = world.get(id).is_some_and(|c| {
            has_passive(c, "voile") && econ_ref(c).is_none_or(|st| time >= st.voile_shield_at)
        });
        if voile {
            apply_shield(
                world,
                ShieldOpts {
                    duration: Some(8.0),
                    id: Some("voile".into()),
                    ..ShieldOpts::new(Some(id), id, 100.0)
                },
            );
            if let Some(c) = world.get_mut(id) {
                econ(c).voile_shield_at = time + 40.0;
            }
            world.emit(GameEvent::BuffApplied {
                target: id,
                buff_id: "item_voile".into(),
                duration: 8.0,
                source: Some(id),
                fx_key: None,
                stacks: None,
            });
        }
        // Fin de stase (sablier).
        if let Some(c) = world.get_mut(id)
            && c.data.contains::<Stasis>()
            && econ(c).stase_until <= time
        {
            c.data.remove::<Stasis>();
        }
    }
}

fn team_of(e: &Entity) -> Option<Team> {
    match e.team {
        TeamOrNeutral::Blue => Some(Team::Blue),
        TeamOrNeutral::Red => Some(Team::Red),
        TeamOrNeutral::Neutral => None,
    }
}

/// Annonce « Le Colosse du Rift charge ! » et demande d'invocation lue par les objectifs.
fn call_colossus(world: &mut World, id: EntityId) {
    let Some(e) = world.get_mut(id) else { return };
    let Some(team) = team_of(e) else { return };
    let (x, y) = (e.pos.x, e.pos.y);
    econ(e).colossus_call = Some(ColossusCall { team, x, y });
    world.emit(GameEvent::Announcement {
        key: "colosse_invoque".into(),
        team: Some(team),
        params: Some(
            [(
                "text".to_owned(),
                ParamValue::Str("Le Colosse du Rift charge !".into()),
            )]
            .into_iter()
            .collect(),
        ),
    });
}

/// Allié vivant le plus proche de `id` à moins de `max_dist`.
fn nearest_ally(world: &World, id: EntityId, max_dist: f64) -> Option<EntityId> {
    let me = world.get(id)?;
    let mut best = None;
    let mut best_d = max_dist;
    for o in world.query(Some(EntityKind::Champion)) {
        if o.id == id || o.team != me.team || o.dead {
            continue;
        }
        let d = dist(me.pos, o.pos);
        if d < best_d {
            best_d = d;
            best = Some(o.id);
        }
    }
    best
}

/// Commande `useItem` : actif d'un objet de l'inventaire (`slot` 0..5) ou du trinket (`slot` -1).
pub fn use_item(world: &mut World, id: EntityId, slot: i32, target_pos: Option<Vec2>) {
    let time = world.time;
    let Some(e) = world.get_mut(id) else { return };
    if e.champ.is_none() || e.dead {
        return;
    }
    // Œil du Héraut, donné par les objectifs hors inventaire.
    if slot == TRINKET_SLOT && e.data.remove::<HeraldEye>().is_some() {
        call_colossus(world, id);
        world.emit(GameEvent::ItemUsed {
            target: id,
            item_id: "colossus_eye".into(),
        });
        return;
    }
    let item_id = if slot == TRINKET_SLOT {
        econ(e).trinket.clone()
    } else {
        inventory_index(slot).and_then(|s| e.champ.as_ref().and_then(|c| c.items[s].clone()))
    };
    let Some(item_id) = item_id else { return };
    let Some(active) = item(&item_id).and_then(|d| d.active.as_ref()) else {
        return;
    };
    if econ(e)
        .item_cooldowns
        .get(&active.id)
        .is_some_and(|&until| until > time)
    {
        return;
    }

    match active.id.as_str() {
        "potion" => {
            let Some(s) = inventory_index(slot) else {
                return;
            };
            let st = econ(e);
            let ch = st.charges[s].unwrap_or(1);
            st.hot = Some(HealOverTime {
                total: 120.0,
                remaining: 15.0,
                duration: 15.0,
            });
            if ch <= 1 {
                st.clear_slot(s);
                if let Some(c) = e.champ.as_mut() {
                    c.items[s] = None;
                }
            } else {
                st.charges[s] = Some(ch - 1);
            }
        }
        "stase" => {
            econ(e).stase_until = time + 2.5;
            // Invulnérable, incoblable, immobile : géré par `combat::buffs`.
            e.data.insert(Stasis);
            world.emit(GameEvent::BuffApplied {
                target: id,
                buff_id: "item_stase".into(),
                duration: 2.5,
                source: Some(id),
                fx_key: None,
                stacks: None,
            });
        }
        "rafale" => {
            econ(e).rafale_until = time + 2.0;
            world.emit(GameEvent::BuffApplied {
                target: id,
                buff_id: "item_rafale".into(),
                duration: 2.0,
                source: Some(id),
                fx_key: None,
                stacks: None,
            });
        }
        "bouclier_allie" => {
            let shield = |target| ShieldOpts {
                duration: Some(2.5),
                id: Some("encensoir".into()),
                ..ShieldOpts::new(Some(id), target, 120.0)
            };
            apply_shield(world, shield(id));
            if let Some(best) = nearest_ally(world, id, 800.0) {
                apply_shield(world, shield(best));
            }
        }
        "frappe_spectrale" => {
            // Dash de 400 u vers la position visée (arrêt au mur) ; annule le rappel.
            let from = e.pos;
            let aim = target_pos.unwrap_or(Vec2 {
                x: from.x + e.facing.cos() * 400.0,
                y: from.y + e.facing.sin() * 400.0,
            });
            let mut to = Vec2 {
                x: clamp(aim.x, 0.0, MAP_SIZE),
                y: clamp(aim.y, 0.0, MAP_SIZE),
            };
            let d = dist(from, to);
            if d > 400.0 {
                to = Vec2 {
                    x: from.x + (to.x - from.x) / d * 400.0,
                    y: from.y + (to.y - from.y) / d * 400.0,
                };
            }
            to = nav_grid().nearest_walkable(to);
            e.data.insert(DashData::new(to, 1400.0));
            if econ(e).recall.take().is_some() {
                world.emit(GameEvent::RecallEnd {
                    target: id,
                    completed: false,
                });
            }
        }
        "soin_rapide" => {
            apply_heal(world, HealOpts::new(Some(id), id, 150.0));
            if let Some(best) = nearest_ally(world, id, 850.0) {
                apply_heal(world, HealOpts::new(Some(id), best, 150.0));
            }
        }
        "colosse" => {
            // L'invocation elle-même est résolue par les objectifs.
            econ(e).trinket = None;
            call_colossus(world, id);
        }
        _ => return,
    }
    if let Some(e) = world.get_mut(id) {
        econ(e)
            .item_cooldowns
            .insert(active.id.clone(), time + active.cooldown);
    }
    world.emit(GameEvent::ItemUsed {
        target: id,
        item_id,
    });
}

/// Lecture de l'état économique pour les autres domaines (`None` si jamais créé).
pub fn economy_state(world: &World, id: EntityId) -> Option<&EconomyState> {
    world.get(id).and_then(econ_ref)
}
