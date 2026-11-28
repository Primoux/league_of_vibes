//! Économie : port de `packages/server/src/systems/economy.ts` (PLAN.md §8).
//!
//! Or (passif, last hits, primes de kill et d'assist, structures, plaques, objectifs), boutique
//! (achat, vente, annulation), rappel (B) et balises (trinket). Les stats et passifs d'objets vivent
//! dans `items/`.
//!
//! Contrats consommés : le hook `bounty` (fourni ici, lu par `combat::damage::kill`), les événements
//! `death`, `structureDestroyed`, `objectiveTaken`, `damage`, `abilityCast`, les commandes `buy`, `sell`,
//! `undoShop`, `recall`, `placeWard`, `useItem`, et les données [`Rewards`], [`PlateReward`],
//! [`LeviathanBuff`] posées par l'IA du monde.
//!
//! Différences avec le TypeScript :
//! - le premier sang (un `WeakMap<World, boolean>` en TS) est un booléen capturé par l'écouteur de mort,
//!   qui n'existe qu'une fois par monde ;
//! - les fonctions désignent les entités par `EntityId` ;
//! - `data.ward` est le type [`WardData`] ;
//! - la vente enregistre les charges du consommable vendu : en TS, annuler la vente d'une pile de
//!   potions rendait l'objet sans ses charges (lues ensuite comme 1). Corrigé ici.

use lov_shared::constants::{
    FOUNTAIN_RADIUS, MAP_SIZE, PASSIVE_GOLD_PER_10S, PASSIVE_GOLD_START, RECALL_SECONDS, scaled,
};
use lov_shared::data::items::item;
use lov_shared::map::{FOUNTAINS, is_in_wall};
use lov_shared::math::{clamp, dist};
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::ItemCategory;
use lov_shared::types::{
    EntityId, EntityKind, GameEvent, MonsterType, PlayerId, Stats, SummonerId, Team, TeamOrNeutral,
    Vec2, WardCharges,
};

use crate::combat::damage::kill;
use crate::items::{
    ConsumedItem, EconomyState, LeviathanBuff, Recall, SLOTS, ShopAction, ShopActionKind,
    TRINKET_SLOT, econ, econ_ref, heal_item_max_hp_delta, inventory_index, item_stats_of,
    register_item_hooks, tick_items, use_item,
};
use crate::systems::vision::{Invisible, RevealedUntil, SightRadius};
use crate::world::{ChampionState, Entity, EventKind, System, World};
use crate::world_ai::runtime::{PlateReward, Rewards};

// ---------------------------------------------------------------- Constantes d'économie

pub const KILL_GOLD: f64 = 300.0;
pub const FIRST_BLOOD_GOLD: f64 = 100.0;
/// Fraction de la prime partagée entre les assistants.
pub const ASSIST_SHARE: f64 = 0.5;
pub const SELL_RATIO: f64 = 0.7;
pub const LOCAL_GOLD_RADIUS: f64 = 1000.0;
/// Or d'équipe d'une structure (repli : l'IA du monde pose [`Rewards`] sur les structures).
pub const STRUCTURE_TEAM_GOLD: f64 = 50.0;
pub const PLATE_GOLD: f64 = 125.0;

/// Or local d'une structure selon son sous-type (repli quand `Rewards::local_gold` est absent).
pub fn structure_local_gold(subtype: &str) -> Option<f64> {
    match subtype {
        "outer" => Some(250.0),
        "inner" | "inhib" => Some(300.0),
        "nexus" => Some(50.0),
        _ => None,
    }
}

/// Or du last hit par type de sbire (repli : `Rewards::gold`, déjà ajusté au temps de jeu).
pub fn minion_gold(subtype: &str) -> Option<f64> {
    match subtype {
        "melee" => Some(21.0),
        "caster" => Some(14.0),
        "cannon" => Some(60.0),
        "super" => Some(90.0),
        _ => None,
    }
}

/// Or du last hit par monstre (repli : `Rewards::gold`).
pub fn monster_gold(subtype: &str) -> Option<f64> {
    match subtype {
        "blue_buff" | "red_buff" | "gromp" => Some(90.0),
        "wolves" | "scuttle" => Some(55.0),
        "wolf_small" | "raptor_small" | "krug_small" => Some(7.0),
        "raptors" => Some(30.0),
        "krugs" => Some(65.0),
        "grub" => Some(20.0),
        _ => None,
    }
}

/// Or d'équipe par objectif épique.
pub fn objective_gold(monster: MonsterType) -> f64 {
    match monster {
        MonsterType::Dragon => 25.0,
        MonsterType::ElderDragon => 250.0,
        MonsterType::Grub => 20.0,
        MonsterType::Herald => 100.0,
        MonsterType::Leviathan => 300.0,
        _ => 0.0,
    }
}

pub const WARD_LIFETIME: f64 = 90.0;
pub const WARD_SIGHT: f64 = 900.0;
pub const WARD_CAST_RANGE: f64 = 600.0;
pub const WARD_MAX_CHARGES: u32 = 2;
pub const WARD_RECHARGE: f64 = 90.0;
/// Une balise meurt en 3 auto-attaques (1 dégât chacune, voir `items::handlers`).
pub const WARD_HP: f64 = 3.0;
/// Rappel renforcé par le buff du Léviathan.
pub const RECALL_LEVIATHAN_SECONDS: f64 = 4.0;

/// `data.ward` : balise posée par un joueur.
#[derive(Debug, Clone, PartialEq)]
pub struct WardData {
    pub owner_id: Option<PlayerId>,
    pub expires_at: f64,
}

/// `Math.round` de JavaScript (demis arrondis vers +infini).
fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

fn other_team(team: Team) -> TeamOrNeutral {
    match team {
        Team::Blue => TeamOrNeutral::Red,
        Team::Red => TeamOrNeutral::Blue,
    }
}

fn fountain_of(team: TeamOrNeutral) -> Vec2 {
    let f = if team == TeamOrNeutral::Red {
        FOUNTAINS.red
    } else {
        FOUNTAINS.blue
    };
    Vec2 { x: f.x, y: f.y }
}

/// Ajoute de l'or à un champion et émet `goldGain` (HUD, FX, audio).
pub fn add_gold(world: &mut World, id: EntityId, amount: f64, pos: Option<Vec2>) {
    if amount <= 0.0 {
        return;
    }
    let Some(c) = world.get_mut(id).and_then(|e| e.champ.as_mut()) else {
        return;
    };
    c.gold += amount;
    world.emit(GameEvent::GoldGain {
        target: id,
        amount: js_round(amount),
        pos,
    });
}

/// Prime de kill d'un champion : base + série de la victime − morts consécutives (minimum 100).
pub fn kill_bounty(victim: &Entity) -> f64 {
    if victim.champ.is_none() {
        return 0.0;
    }
    let (kill_streak, death_streak) =
        econ_ref(victim).map_or((0, 0), |st| (st.kill_streak, st.death_streak));
    let mut b = KILL_GOLD;
    if kill_streak >= 5 {
        b += 300.0;
    } else if kill_streak >= 3 {
        b += 150.0;
    }
    b -= f64::from(death_streak.min(4)) * 25.0;
    b.max(100.0)
}

/// Boutique : achat possible à sa fontaine, ou en étant mort.
pub fn can_shop_now(e: &Entity) -> bool {
    e.dead || dist(e.pos, fountain_of(e.team)) <= FOUNTAIN_RADIUS
}

fn has_smite(e: &Entity) -> bool {
    e.champ
        .as_ref()
        .is_some_and(|c| c.summoners.d == SummonerId::Smite || c.summoners.f == SummonerId::Smite)
}

// ---------------------------------------------------------------- Boutique

fn owns_item(e: &Entity, item_id: &str) -> bool {
    e.champ
        .as_ref()
        .is_some_and(|c| c.items.iter().any(|s| s.as_deref() == Some(item_id)))
        || econ_ref(e).is_some_and(|st| st.trinket.as_deref() == Some(item_id))
}

fn slot_of(e: &Entity, item_id: &str) -> Option<usize> {
    e.champ
        .as_ref()?
        .items
        .iter()
        .position(|s| s.as_deref() == Some(item_id))
}

fn buy_item(world: &mut World, id: EntityId, item_id: &str) {
    let Some(def) = item(item_id) else { return };
    let Some(e) = world.get_mut(id) else { return };
    if def.shop == Some(false) || e.champ.is_none() || !can_shop_now(e) {
        return;
    }

    if def.category == ItemCategory::Trinket {
        let st = econ(e);
        if st.trinket.is_some() {
            return;
        }
        st.trinket = Some(def.id.clone());
        if def.id == "stealth_ward" {
            st.trinket_charges = WARD_MAX_CHARGES;
            st.trinket_recharge_at = 0.0;
        }
        st.shop_history.push(ShopAction {
            kind: ShopActionKind::Buy,
            item_id: def.id.clone(),
            slot: TRINKET_SLOT,
            gold: 0.0,
            paid_price: 0.0,
            consumed: Vec::new(),
            charges: None,
        });
        world.emit(GameEvent::ItemBought {
            target: id,
            item_id: def.id.clone(),
        });
        return;
    }

    if def.unique == Some(true) && owns_item(e, &def.id) {
        return;
    }

    // Consommable : on empile dans l'emplacement existant tant qu'il reste de la place.
    if let Some(max_stacks) = def.max_stacks
        && let Some(existing) = slot_of(e, &def.id)
    {
        let gold = e.champ.as_ref().map_or(0.0, |c| c.gold);
        let st = econ(e);
        let ch = st.charges[existing].unwrap_or(1);
        if ch < max_stacks && gold >= def.cost {
            st.charges[existing] = Some(ch + 1);
            // Prix payé cumulé : la vente rembourse 70 % du total.
            let paid = st.paid_price[existing].unwrap_or(0.0) + def.cost;
            st.paid_price[existing] = Some(paid);
            st.shop_history.push(ShopAction {
                kind: ShopActionKind::Buy,
                item_id: def.id.clone(),
                slot: existing as i32,
                gold: def.cost,
                paid_price: paid,
                consumed: Vec::new(),
                charges: Some(ch),
            });
            if let Some(c) = e.champ.as_mut() {
                c.gold -= def.cost;
            }
            world.emit(GameEvent::ItemBought {
                target: id,
                item_id: def.id.clone(),
            });
        }
        return;
    }

    // Recette : les composants possédés sont consommés et leur coût est déduit. Un composant demandé
    // deux fois (`grand_glaive` = 2 × `long_sword`) prend deux emplacements distincts.
    let mut consumed: Vec<ConsumedItem> = Vec::new();
    let mut reduction = 0.0;
    for comp in &def.builds_from {
        let free = e.champ.as_ref().and_then(|c| {
            c.items.iter().enumerate().position(|(i, s)| {
                s.as_deref() == Some(comp.as_str()) && !consumed.iter().any(|u| u.slot == i)
            })
        });
        if let Some(cslot) = free {
            let comp_cost = item(comp).map_or(0.0, |d| d.cost);
            let paid = econ_ref(e)
                .and_then(|st| st.paid_price[cslot])
                .unwrap_or(comp_cost);
            consumed.push(ConsumedItem {
                slot: cslot,
                item_id: comp.clone(),
                paid_price: paid,
            });
            reduction += comp_cost;
        }
    }
    let cost = (def.cost - reduction).max(0.0);
    let Some(c) = e.champ.as_mut() else { return };
    let Some(slot) = c.items.iter().position(Option::is_none) else {
        return;
    };
    if c.gold < cost {
        return;
    }
    c.gold -= cost;
    for comp in &consumed {
        c.items[comp.slot] = None;
    }
    c.items[slot] = Some(def.id.clone());
    let st = econ(e);
    for comp in &consumed {
        st.clear_slot(comp.slot);
    }
    let paid = cost + consumed.iter().map(|c| c.paid_price).sum::<f64>();
    if slot < SLOTS {
        st.paid_price[slot] = Some(paid);
        if def.max_stacks.is_some() {
            st.charges[slot] = Some(1);
        }
    }
    st.shop_history.push(ShopAction {
        kind: ShopActionKind::Buy,
        item_id: def.id.clone(),
        slot: slot as i32,
        gold: cost,
        paid_price: paid,
        consumed,
        charges: Some(0),
    });
    world.emit(GameEvent::ItemBought {
        target: id,
        item_id: def.id.clone(),
    });
}

fn sell_item(world: &mut World, id: EntityId, slot: i32) {
    let Some(e) = world.get_mut(id) else { return };
    let Some(s) = inventory_index(slot) else {
        return;
    };
    if !can_shop_now(e) {
        return;
    }
    let Some(item_id) = e
        .champ
        .as_ref()
        .and_then(|c| c.items.get(s).cloned().flatten())
    else {
        return;
    };
    let st = econ(e);
    let paid = st.paid_price[s]
        .or_else(|| item(&item_id).map(|d| d.cost))
        .unwrap_or(0.0);
    let refund = js_round(paid * SELL_RATIO);
    let charges = st.charges[s];
    st.clear_slot(s);
    st.shop_history.push(ShopAction {
        kind: ShopActionKind::Sell,
        item_id: item_id.clone(),
        slot,
        gold: refund,
        paid_price: paid,
        consumed: Vec::new(),
        charges,
    });
    if let Some(c) = e.champ.as_mut() {
        c.gold += refund;
        c.items[s] = None;
    }
    world.emit(GameEvent::ItemSold {
        target: id,
        item_id,
    });
}

/// Annule la dernière action de boutique (tant qu'on est encore à la fontaine).
fn undo_shop(world: &mut World, id: EntityId) {
    let Some(Entity { champ, data, .. }) = world.get_mut(id) else {
        return;
    };
    let Some(c) = champ.as_mut() else { return };
    let st = data.get_or_insert_with(EconomyState::default);
    if !st.can_shop {
        return;
    }
    let Some(act) = st.shop_history.pop() else {
        return;
    };
    match act.kind {
        ShopActionKind::Buy => {
            undo_buy(c, st, &act);
            c.gold += act.gold;
        }
        ShopActionKind::Sell => {
            if c.gold < act.gold {
                // Pas assez d'or pour racheter : l'annulation est elle-même annulée.
                st.shop_history.push(act);
                return;
            }
            c.gold -= act.gold;
            if act.slot == TRINKET_SLOT {
                st.trinket = Some(act.item_id);
            } else if let Some(s) = inventory_index(act.slot) {
                c.items[s] = Some(act.item_id);
                st.paid_price[s] = Some(act.paid_price);
                if act.charges.is_some() {
                    st.charges[s] = act.charges;
                }
            }
        }
    }
}

fn undo_buy(c: &mut ChampionState, st: &mut EconomyState, act: &ShopAction) {
    if act.slot == TRINKET_SLOT {
        st.trinket = None;
        st.trinket_charges = 0;
        return;
    }
    let Some(s) = inventory_index(act.slot) else {
        return;
    };
    let stackable = item(&act.item_id).is_some_and(|d| d.max_stacks.is_some());
    if stackable
        && c.items[s].as_deref() == Some(act.item_id.as_str())
        && let Some(charges) = act.charges
    {
        // Annulation d'un achat empilé : on rend la charge, pas toute la pile.
        if charges == 0 {
            c.items[s] = None;
            st.clear_slot(s);
        } else {
            st.charges[s] = Some(charges);
            // Le prix payé redescend à celui d'avant cet achat.
            st.paid_price[s] = Some(act.paid_price - act.gold);
        }
        return;
    }
    c.items[s] = None;
    st.clear_slot(s);
    for comp in &act.consumed {
        if let Some(free) = c.items.iter().position(Option::is_none) {
            c.items[free] = Some(comp.item_id.clone());
            if free < SLOTS {
                st.paid_price[free] = Some(comp.paid_price);
            }
        }
    }
}

// ---------------------------------------------------------------- Rappel et balises

fn start_recall(world: &mut World, id: EntityId) {
    let time = world.time;
    let Some(e) = world.get_mut(id) else { return };
    if e.dead || econ(e).recall.is_some() {
        return;
    }
    // Buff du Léviathan : rappel renforcé.
    let total = match e.data.get::<LeviathanBuff>() {
        Some(lb) if lb.expires_at > time => lb.recall_seconds.unwrap_or(RECALL_LEVIATHAN_SECONDS),
        _ => RECALL_SECONDS,
    };
    econ(e).recall = Some(Recall {
        remaining: total,
        total,
    });
    world.emit(GameEvent::RecallStart { target: id });
}

fn cancel_recall(world: &mut World, id: EntityId) {
    let Some(e) = world.get_mut(id) else { return };
    let Some(st) = e.data.get_mut::<EconomyState>() else {
        return;
    };
    if st.recall.take().is_none() {
        return;
    }
    world.emit(GameEvent::RecallEnd {
        target: id,
        completed: false,
    });
}

fn ward_stats() -> Stats {
    Stats {
        max_hp: WARD_HP,
        hp_regen: 0.0,
        max_mana: 0.0,
        mana_regen: 0.0,
        ad: 0.0,
        ap: 0.0,
        armor: 0.0,
        mr: 0.0,
        attack_speed: 0.0,
        attack_range: 0.0,
        move_speed: 0.0,
        crit_chance: 0.0,
        ability_haste: 0.0,
        lifesteal: 0.0,
        armor_pen: 0.0,
        magic_pen: 0.0,
    }
}

fn place_ward(world: &mut World, id: EntityId, pos: Vec2) {
    let time = world.time;
    let Some(e) = world.get_mut(id) else { return };
    if e.dead {
        return;
    }
    {
        let st = econ(e);
        if st.trinket.as_deref() != Some("stealth_ward") || st.trinket_charges == 0 {
            return;
        }
    }
    let p = Vec2 {
        x: clamp(pos.x, 0.0, MAP_SIZE),
        y: clamp(pos.y, 0.0, MAP_SIZE),
    };
    if dist(e.pos, p) > WARD_CAST_RANGE || is_in_wall(p) {
        return;
    }
    let team = e.team;
    let owner_id = e.champ.as_ref().map(|c| c.player_id.clone());
    let st = econ(e);
    st.trinket_charges -= 1;
    if st.trinket_charges < WARD_MAX_CHARGES {
        st.trinket_recharge_at = time + WARD_RECHARGE;
    }
    let mut ward = Entity::new(EntityKind::Ward, team, p, 30.0, WARD_HP, 0.0, ward_stats());
    ward.subtype = Some("stealth_ward".into());
    // Invisible pour l'équipe adverse (vision).
    ward.data.insert(Invisible);
    ward.data.insert(SightRadius(WARD_SIGHT));
    ward.data.insert(WardData {
        owner_id,
        expires_at: time + WARD_LIFETIME,
    });
    world.spawn(ward);
    world.emit(GameEvent::ItemUsed {
        target: id,
        item_id: "stealth_ward".into(),
    });
}

// ---------------------------------------------------------------- Récompenses

fn grant_kill_gold(
    world: &mut World,
    first_blood: &mut bool,
    target: EntityId,
    killer: Option<EntityId>,
    assists: &[EntityId],
    bounty: Option<f64>,
) {
    let Some(t) = world.get(target) else { return };
    let (t_kind, t_team, t_pos, t_is_champ) = (t.kind, t.team, t.pos, t.champ.is_some());
    let killer_info = world
        .get_opt(killer)
        .map(|k| (k.id, k.team, k.champ.is_some()));

    if t_kind == EntityKind::Champion && t_is_champ {
        if let Some(t) = world.get_mut(target) {
            let st = econ(t);
            st.death_streak += 1;
            st.kill_streak = 0;
        }
        cancel_recall(world, target);
        if let Some((kid, kteam, true)) = killer_info
            && kteam != t_team
        {
            if let Some(k) = world.get_mut(kid) {
                econ(k).kill_streak += 1;
            }
            let mut gold = bounty.unwrap_or(KILL_GOLD);
            if !*first_blood {
                *first_blood = true;
                // Pas d'annonce ici : l'interface dérive premier sang, séries et multi-kills de `death`.
                gold += FIRST_BLOOD_GOLD;
            }
            add_gold(world, kid, gold, Some(t_pos));
        }
        if let Some(bounty) = bounty {
            let helpers: Vec<EntityId> = assists
                .iter()
                .copied()
                .filter(|&a| world.get(a).is_some_and(|a| a.champ.is_some()))
                .collect();
            if !helpers.is_empty() {
                let share = bounty * ASSIST_SHARE / helpers.len() as f64;
                for a in helpers {
                    add_gold(world, a, share, Some(t_pos));
                }
            }
        }
        return;
    }

    // Last hit sur sbire ou monstre : or et CS pour le champion tueur.
    if matches!(t_kind, EntityKind::Minion | EntityKind::Monster)
        && let Some((kid, kteam, true)) = killer_info
        && kteam != t_team
    {
        let Some(t) = world.get(target) else { return };
        let rw = t.data.get::<Rewards>().copied();
        let subtype = t.subtype.as_deref();
        let mut gold = rw.and_then(|r| r.gold).unwrap_or_else(|| {
            if t_kind == EntityKind::Minion {
                minion_gold(subtype.unwrap_or("melee")).unwrap_or(21.0)
            } else {
                monster_gold(subtype.unwrap_or("")).unwrap_or(50.0)
            }
        });
        // Camps de jungle : or complet uniquement avec Châtiment (PLAN §7).
        let smite_ok = world.get(kid).is_some_and(has_smite);
        if rw.and_then(|r| r.smite_required) == Some(true) && !smite_ok {
            gold = js_round(gold / 2.0);
        }
        if let Some(c) = world.get_mut(kid).and_then(|k| k.champ.as_mut()) {
            c.cs += 1;
        }
        add_gold(world, kid, gold, Some(t_pos));
    }
}

fn champions_of(world: &World, team: TeamOrNeutral, out: &mut Vec<(EntityId, Vec2)>) {
    out.clear();
    out.extend(
        world
            .query(Some(EntityKind::Champion))
            .filter(|c| c.champ.is_some() && c.team == team)
            .map(|c| (c.id, c.pos)),
    );
}

fn grant_structure_gold(world: &mut World, target: EntityId, team: Team, plate: bool) {
    let st = world.get(target);
    let pos = st.map_or(Vec2 { x: 0.0, y: 0.0 }, |s| s.pos);
    // L'équipe qui a détruit la structure.
    let winners = other_team(team);
    let mut champs = Vec::new();
    champions_of(world, winners, &mut champs);
    if plate {
        // Plaque : or local, lu dans `PlateReward` posé par l'IA du monde pendant l'événement.
        let pr = st.and_then(|s| s.data.get::<PlateReward>()).copied();
        let gold = pr.map_or(PLATE_GOLD, |p| p.gold);
        let radius = pr.map_or(LOCAL_GOLD_RADIUS, |p| p.radius);
        for (id, cpos) in champs {
            if dist(cpos, pos) <= radius {
                add_gold(world, id, gold, Some(pos));
            }
        }
        return;
    }
    let rw = st.and_then(|s| s.data.get::<Rewards>()).copied();
    let team_gold = rw.and_then(|r| r.team_gold).unwrap_or(STRUCTURE_TEAM_GOLD);
    let local_gold = rw.and_then(|r| r.local_gold).unwrap_or_else(|| {
        st.and_then(|s| s.subtype.as_deref())
            .and_then(structure_local_gold)
            .unwrap_or(250.0)
    });
    for (id, cpos) in champs {
        add_gold(world, id, team_gold, None);
        if dist(cpos, pos) <= LOCAL_GOLD_RADIUS {
            add_gold(world, id, local_gold, None);
        }
    }
}

fn grant_objective_gold(world: &mut World, monster: MonsterType, team: Team) {
    let gold = objective_gold(monster);
    if gold == 0.0 {
        return;
    }
    let mut champs = Vec::new();
    let team = match team {
        Team::Blue => TeamOrNeutral::Blue,
        Team::Red => TeamOrNeutral::Red,
    };
    champions_of(world, team, &mut champs);
    for (id, _) in champs {
        add_gold(world, id, gold, None);
    }
}

// ---------------------------------------------------------------- Système

fn init_economy(world: &mut World) {
    register_item_hooks(world);

    // Prime de kill lue par `combat::damage::kill` (champ `bounty` de l'événement `death`).
    world.hooks.bounty.push(Box::new(|world, victim, _, _| {
        world
            .get(victim)
            .filter(|v| v.champ.is_some())
            .map(kill_bounty)
    }));

    // Rappel : immobile pendant la canalisation.
    world.hooks.can_move.push(Box::new(|world, id| {
        world
            .get(id)
            .and_then(econ_ref)
            .is_none_or(|st| st.recall.is_none())
    }));

    let mut first_blood = false;
    world.events.on(EventKind::Death, move |world, ev| {
        if let GameEvent::Death {
            target,
            killer,
            assists,
            bounty,
        } = ev
        {
            let assists = assists.as_deref().unwrap_or(&[]);
            grant_kill_gold(world, &mut first_blood, *target, *killer, assists, *bounty);
        }
    });
    world.events.on(EventKind::StructureDestroyed, |world, ev| {
        if let GameEvent::StructureDestroyed {
            target,
            team,
            plate,
            ..
        } = ev
        {
            grant_structure_gold(world, *target, *team, plate.unwrap_or(false));
        }
    });
    world.events.on(EventKind::ObjectiveTaken, |world, ev| {
        if let GameEvent::ObjectiveTaken { monster, team, .. } = ev {
            grant_objective_gold(world, *monster, *team);
        }
    });

    // L'interface attend `champ.items` de longueur 7 dans le snapshot (index 6 = trinket).
    world.hooks.entity_snapshot.push(Box::new(|_, e, s| {
        let (Some(c), Some(sc)) = (&e.champ, s.champ.as_mut()) else {
            return;
        };
        sc.items.clone_from(&c.items);
        sc.items.push(econ_ref(e).and_then(|st| st.trinket.clone()));
    }));

    // État privé envoyé au joueur.
    world.hooks.self_state.push(Box::new(|world, e, s| {
        if e.champ.is_none() {
            return;
        }
        let Some(st) = econ_ref(e) else { return };
        s.can_shop = st.can_shop;
        if let Some(r) = st.recall {
            s.recalling = Some(r.remaining.max(0.0));
        }
        if st.trinket.as_deref() == Some("stealth_ward") {
            s.ward_charges = Some(WardCharges {
                charges: st.trinket_charges,
                cooldown_remaining: (st.trinket_recharge_at - world.time).max(0.0),
            });
        }
    }));

    // Le rappel est interrompu par les dégâts subis.
    world.events.on(EventKind::Damage, |world, ev| {
        if let GameEvent::Damage { target, .. } = ev
            && world.get(*target).is_some_and(|t| t.champ.is_some())
        {
            cancel_recall(world, *target);
        }
    });
}

/// Applique une commande de boutique en soignant le delta de PV max des objets.
fn with_item_hp_delta(world: &mut World, id: EntityId, f: impl FnOnce(&mut World)) {
    let Some(before) = world.get(id).map(item_stats_of) else {
        return;
    };
    f(world);
    if let Some(e) = world.get_mut(id) {
        heal_item_max_hp_delta(e, &before);
    }
}

fn update_economy(world: &mut World, dt: f64, ids: &mut Vec<EntityId>) {
    // Commandes de boutique, de rappel, de balise et d'objets actifs.
    for i in 0..world.commands.len() {
        let msg = world.commands[i].msg.clone();
        let Some(id) = world.champion_id_of(&world.commands[i].player_id) else {
            continue;
        };
        let Some(dead) = world.get(id).filter(|e| e.champ.is_some()).map(|e| e.dead) else {
            continue;
        };
        match msg {
            ClientMsg::Buy { item_id } => {
                with_item_hp_delta(world, id, |w| buy_item(w, id, &item_id));
            }
            ClientMsg::Sell { slot } => with_item_hp_delta(world, id, |w| sell_item(w, id, slot)),
            ClientMsg::UndoShop => with_item_hp_delta(world, id, |w| undo_shop(w, id)),
            ClientMsg::Recall => start_recall(world, id),
            ClientMsg::PlaceWard { pos } => place_ward(world, id, pos),
            ClientMsg::UseItem { slot, target_pos } => use_item(world, id, slot, target_pos),
            ClientMsg::Move { .. } | ClientMsg::AttackMove { .. } | ClientMsg::Attack { .. }
                if !dead =>
            {
                cancel_recall(world, id);
            }
            _ => {}
        }
    }

    let time = world.time;
    world.ids_into(Some(EntityKind::Champion), ids);
    for &id in ids.iter() {
        // Or passif (dès `scaled(PASSIVE_GOLD_START)`, toutes les 10 s, même mort).
        let Some(c) = world.get_mut(id) else { continue };
        let st = econ(c);
        if st.next_passive_gold_at == 0.0 {
            st.next_passive_gold_at = scaled(PASSIVE_GOLD_START);
        }
        while world
            .get_mut(id)
            .is_some_and(|c| time >= econ(c).next_passive_gold_at)
        {
            add_gold(world, id, PASSIVE_GOLD_PER_10S, None);
            if let Some(c) = world.get_mut(id) {
                econ(c).next_passive_gold_at += 10.0;
            }
        }

        let Some(c) = world.get_mut(id) else { continue };
        // Achat autorisé : fontaine ou mort. Quitter la fontaine vide l'historique d'annulation.
        let can_shop = can_shop_now(c);
        let dead = c.dead;
        let team = c.team;
        let st = econ(c);
        st.can_shop = can_shop;
        if !can_shop {
            st.shop_history.clear();
        }

        // Rappel en cours : `Some(true)` quand la canalisation est terminée.
        let recall_done = st.recall.as_mut().map(|r| {
            r.remaining -= dt;
            r.remaining <= 0.0
        });
        if recall_done.is_some() && dead {
            cancel_recall(world, id);
        } else if recall_done == Some(true) {
            let f = fountain_of(team);
            if let Some(c) = world.get_mut(id) {
                c.pos = Vec2 {
                    x: f.x + 60.0,
                    y: f.y + 60.0,
                };
                c.move_target = None;
                c.path = None;
                econ(c).recall = None;
            }
            world.emit(GameEvent::RecallEnd {
                target: id,
                completed: true,
            });
        }

        // Recharge du trinket de vision.
        if let Some(c) = world.get_mut(id) {
            let st = econ(c);
            if st.trinket.as_deref() == Some("stealth_ward")
                && st.trinket_charges < WARD_MAX_CHARGES
                && time >= st.trinket_recharge_at
            {
                st.trinket_charges += 1;
                if st.trinket_charges < WARD_MAX_CHARGES {
                    st.trinket_recharge_at = time + WARD_RECHARGE;
                }
            }
        }
    }

    tick_items(world, dt, ids);

    // Balises : expiration, et révélation quand un ennemi entre dans leur rayon.
    world.ids_into(Some(EntityKind::Ward), ids);
    for &wid in ids.iter() {
        let Some(w) = world.get(wid) else { continue };
        if w.data
            .get::<WardData>()
            .is_some_and(|wd| time >= wd.expires_at)
        {
            kill(world, wid, None);
            continue;
        }
        let (wteam, wpos) = (w.team, w.pos);
        let spotted = world
            .query(Some(EntityKind::Champion))
            .any(|c| c.team != wteam && !c.dead && dist(c.pos, wpos) <= WARD_SIGHT);
        if spotted && let Some(w) = world.get_mut(wid) {
            w.data.insert(RevealedUntil(time + 1.0));
        }
    }
}

/// Le système économie.
pub fn economy_system() -> System {
    // Tampon d'ids réutilisé d'un tick à l'autre.
    let mut ids: Vec<EntityId> = Vec::new();
    System::new("economy", move |world, dt| {
        update_economy(world, dt, &mut ids)
    })
    .with_init(init_economy)
}
