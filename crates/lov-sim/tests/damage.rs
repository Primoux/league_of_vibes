//! Tests de `combat/damage` : port de `packages/server/test/damage.test.ts`.
//!
//! Le monde de test ne contient aucun des systèmes de `coreWorld()` (`input`, `movement`, `combat`,
//! `vision`) : les dégâts n'en ont pas besoin. Seuls deux tests dépendent du système `combat` du TS
//! (expiration des boucliers et minuteur de réapparition, tous deux dans sa boucle `update`) : il n'est
//! pas encore porté, donc `combat_shim` reproduit ces deux lignes en appelant les vraies fonctions de
//! `damage.rs` (`expire_shields`, `respawn_champion`), dans le même ordre que `combat.ts`.

use lov_shared::types::{
    DamageType, EntityId, EntityKind, GameEvent, Stats, SummonerId, Team, TeamOrNeutral, Vec2,
};
use lov_sim::combat::damage::{
    DamageOpts, HealOpts, Owner, ShieldOpts, apply_damage, apply_heal, apply_shield,
    default_respawn_time, expire_shields, respawn_champion,
};
use lov_sim::{CreateChampion, Entity, System, World, create_champion};

fn v(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

fn close(a: f64, b: f64) {
    // `toBeCloseTo` de vitest : 2 décimales.
    assert!((a - b).abs() < 0.005, "{a} != {b}");
}

/// `BASE_STATS` de `helpers.ts`.
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

/// Stats de base de grondak (`packages/shared/src/data/champions/grondak.ts`), le champion de `champ()`.
fn grondak_stats() -> Stats {
    Stats {
        max_hp: 680.0,
        max_mana: 0.0,
        ad: 64.0,
        armor: 38.0,
        mr: 32.0,
        attack_speed: 0.62,
        move_speed: 335.0,
        ..base_stats()
    }
}

/// Partie des mises à jour de `combat.ts` dont dépendent les tests (voir l'en-tête du fichier).
fn combat_shim() -> System {
    let mut ids: Vec<EntityId> = Vec::new();
    System::new("combat-shim", move |world, dt| {
        let time = world.time;
        world.ids_into(None, &mut ids);
        for &id in &ids {
            let Some(e) = world.get_mut(id) else { continue };
            expire_shields(time, e);
            if e.dead
                && let Some(c) = e.champ.as_mut()
            {
                c.respawn_in -= dt;
                if c.respawn_in <= 0.0 {
                    respawn_champion(world, id);
                }
            }
        }
    })
}

struct Sim {
    world: World,
    systems: [System; 1],
    n: u32,
}

impl Sim {
    /// `coreWorld()`.
    fn new() -> Self {
        let mut systems = [combat_shim()];
        let mut world = World::new(1);
        world.init(&mut systems);
        Self {
            world,
            systems,
            n: 0,
        }
    }

    /// `run(world, seconds)` : `round(seconds * 30)` ticks.
    fn run(&mut self, seconds: f64) {
        for _ in 0..(seconds * 30.0).round() as u32 {
            self.world.step(&mut self.systems);
        }
    }

    fn champ(&mut self, team: Team, pos: Vec2) -> EntityId {
        self.n += 1;
        let opts = CreateChampion {
            player_id: format!("p{}", self.n),
            name: "T".into(),
            team,
            champion_id: "grondak".into(),
            summoners: [SummonerId::Flash, SummonerId::Ignite],
            pos,
        };
        create_champion(&mut self.world, opts, &grondak_stats())
    }

    fn dummy(
        &mut self,
        team: TeamOrNeutral,
        pos: Vec2,
        tweak: impl FnOnce(&mut Stats),
    ) -> EntityId {
        self.dummy_kind(team, pos, tweak, EntityKind::Minion)
    }

    fn dummy_kind(
        &mut self,
        team: TeamOrNeutral,
        pos: Vec2,
        tweak: impl FnOnce(&mut Stats),
        kind: EntityKind,
    ) -> EntityId {
        let mut s = base_stats();
        tweak(&mut s);
        let hp = s.max_hp;
        self.world
            .spawn(Entity::new(kind, team, pos, 40.0, hp, 0.0, s))
    }

    fn e(&self, id: EntityId) -> &Entity {
        self.world.get(id).unwrap()
    }

    fn dmg(
        &mut self,
        source: Option<EntityId>,
        target: EntityId,
        amount: f64,
        t: DamageType,
    ) -> f64 {
        apply_damage(&mut self.world, DamageOpts::new(source, target, amount, t))
    }
}

#[test]
fn applique_armure_et_rm_les_degats_bruts_ignorent_les_resistances() {
    let mut s = Sim::new();
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |st| {
        st.armor = 100.0;
        st.mr = 50.0;
    });
    close(s.dmg(None, t, 100.0, DamageType::Physical), 50.0);
    close(s.dmg(None, t, 150.0, DamageType::Magic), 100.0);
    close(s.dmg(None, t, 100.0, DamageType::True), 100.0);
    close(s.e(t).hp, 1000.0 - 250.0);
    let evs = s
        .world
        .events
        .drain()
        .into_iter()
        .filter(|e| matches!(e, GameEvent::Damage { .. }))
        .count();
    assert_eq!(evs, 3);
}

#[test]
fn applique_la_penetration_fixe_sans_descendre_sous_0() {
    let mut s = Sim::new();
    let src = s.dummy(TeamOrNeutral::Blue, v(0.0, 0.0), |st| {
        st.armor_pen = 50.0;
        st.magic_pen = 200.0;
    });
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |st| {
        st.armor = 150.0;
        st.mr = 50.0;
    });
    close(s.dmg(Some(src), t, 200.0, DamageType::Physical), 100.0);
    close(s.dmg(Some(src), t, 100.0, DamageType::Magic), 100.0);
}

#[test]
fn les_resistances_negatives_amplifient_les_degats() {
    let mut s = Sim::new();
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |st| {
        st.armor = -100.0
    });
    close(s.dmg(None, t, 100.0, DamageType::Physical), 150.0);
}

#[test]
fn consomme_les_boucliers_avant_les_pv_et_les_fait_expirer() {
    let mut s = Sim::new();
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |_| {});
    let mut o = ShieldOpts::new(Some(t), t, 80.0);
    o.duration = Some(2.0);
    apply_shield(&mut s.world, o);
    apply_shield(&mut s.world, ShieldOpts::new(Some(t), t, 50.0));
    assert_eq!(s.e(t).shield, 130.0);
    let dealt = s.dmg(None, t, 100.0, DamageType::True);
    assert_eq!(dealt, 100.0);
    assert_eq!(s.e(t).hp, 1000.0); // 80 (expire en premier) + 20 sur le permanent
    assert_eq!(s.e(t).shield, 30.0);
    let mut o = ShieldOpts::new(Some(t), t, 40.0);
    o.duration = Some(1.0);
    apply_shield(&mut s.world, o);
    s.run(1.1);
    assert_eq!(s.e(t).shield, 30.0);
}

#[test]
fn tue_emet_death_avec_killer_et_assists_met_a_jour_kda() {
    let mut s = Sim::new();
    let a = s.champ(Team::Blue, v(6000.0, 3000.0));
    let b = s.champ(Team::Blue, v(6100.0, 3000.0));
    let vic = s.champ(Team::Red, v(6000.0, 3200.0));
    s.world
        .hooks
        .bounty
        .push(Box::new(|_, _, _, _| Some(300.0)));
    s.dmg(Some(b), vic, 100.0, DamageType::True);
    s.dmg(Some(a), vic, 1e6, DamageType::True);
    assert!(s.e(vic).dead);
    assert_eq!(s.e(vic).champ.as_ref().unwrap().deaths, 1);
    assert_eq!(s.e(a).champ.as_ref().unwrap().kills, 1);
    assert_eq!(s.e(b).champ.as_ref().unwrap().assists, 1);
    assert!(s.e(a).champ.as_ref().unwrap().damage_to_champions > 500.0);
    let death = s
        .world
        .events
        .drain()
        .into_iter()
        .find(|e| matches!(e, GameEvent::Death { .. }));
    assert_eq!(
        death,
        Some(GameEvent::Death {
            target: vic,
            killer: Some(a),
            assists: Some(vec![b]),
            bounty: Some(300.0),
        })
    );
    close(
        s.e(vic).champ.as_ref().unwrap().respawn_in,
        default_respawn_time(1, s.world.time),
    );
}

#[test]
fn credite_le_kill_au_champion_si_une_tour_porte_le_coup_final() {
    let mut s = Sim::new();
    let a = s.champ(Team::Blue, v(6000.0, 3000.0));
    let tower = s.dummy_kind(
        TeamOrNeutral::Blue,
        v(6000.0, 2500.0),
        |_| {},
        EntityKind::Tower,
    );
    let vic = s.champ(Team::Red, v(6000.0, 3200.0));
    s.dmg(Some(a), vic, 100.0, DamageType::True);
    s.dmg(Some(tower), vic, 1e6, DamageType::True);
    assert_eq!(s.e(a).champ.as_ref().unwrap().kills, 1);
    let death = s
        .world
        .events
        .drain()
        .into_iter()
        .find(|e| matches!(e, GameEvent::Death { .. }));
    assert!(matches!(
        death,
        Some(GameEvent::Death { killer, .. }) if killer == Some(a)
    ));
}

#[test]
fn respawn_a_la_fontaine_apres_le_timer() {
    let mut s = Sim::new();
    let vic = s.champ(Team::Red, v(6000.0, 6000.0));
    s.dmg(None, vic, 1e6, DamageType::True);
    assert!(s.e(vic).dead);
    let respawn_in = s.e(vic).champ.as_ref().unwrap().respawn_in;
    s.run(respawn_in + 0.1);
    assert!(!s.e(vic).dead);
    assert_eq!(s.e(vic).hp, s.e(vic).stats.max_hp);
    assert!(s.e(vic).pos.x > 11000.0);
    assert!(
        s.world
            .events
            .drain()
            .iter()
            .any(|e| matches!(e, GameEvent::Respawn { .. }))
    );
}

#[test]
fn hooks_before_damage_reduction_et_prevent_death() {
    let mut s = Sim::new();
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |_| {});
    s.world
        .hooks
        .before_damage
        .push(Box::new(|_, ctx| ctx.multiplier *= 0.5));
    assert_eq!(s.dmg(None, t, 100.0, DamageType::True), 50.0);
    s.world.hooks.prevent_death.push(Box::new(|w, id, _| {
        if let Some(e) = w.get_mut(id) {
            e.hp = 1.0;
        }
        true
    }));
    s.dmg(None, t, 1e6, DamageType::True);
    assert!(!s.e(t).dead);
    assert_eq!(s.e(t).hp, 1.0);
}

#[test]
fn soins_plafonnes_aux_pv_max_lifesteal_sur_aa_uniquement() {
    let mut s = Sim::new();
    let src = s.dummy(TeamOrNeutral::Blue, v(0.0, 0.0), |st| st.lifesteal = 0.5);
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |_| {});
    s.world.get_mut(src).unwrap().hp = 500.0;
    assert_eq!(
        apply_heal(&mut s.world, HealOpts::new(Some(src), src, 2000.0)),
        500.0
    );
    s.world.get_mut(src).unwrap().hp = 500.0;
    s.dmg(Some(src), t, 100.0, DamageType::Physical);
    assert_eq!(s.e(src).hp, 500.0);
    let mut o = DamageOpts::new(Some(src), t, 100.0, DamageType::Physical);
    o.is_auto_attack = true;
    apply_damage(&mut s.world, o);
    assert_eq!(s.e(src).hp, 550.0);
}

#[test]
fn credite_le_proprietaire_data_owner() {
    let mut s = Sim::new();
    let a = s.champ(Team::Blue, v(6000.0, 3000.0));
    let pet = s.dummy(TeamOrNeutral::Blue, v(6000.0, 3000.0), |_| {});
    s.world.get_mut(pet).unwrap().data.insert(Owner(a));
    let vic = s.champ(Team::Red, v(6000.0, 3200.0));
    s.dmg(Some(pet), vic, 1e6, DamageType::True);
    assert_eq!(s.e(a).champ.as_ref().unwrap().kills, 1);
}
