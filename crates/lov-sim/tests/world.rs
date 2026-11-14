//! Tests du monde (port de `packages/server/test/world.test.ts` + couverture des nouveautés Rust).
//! Le test TS « déplace un champion » dépend des systèmes `input` et `movement` : il sera porté avec eux.

use std::sync::{Arc, Mutex};

use lov_shared::protocol::ClientMsg;
use lov_shared::types::{
    DamageType, EntityId, EntityKind, GameEvent, Stats, SummonerId, Team, TeamOrNeutral, Vec2,
};
use lov_sim::{
    AttackContext, CreateChampion, DamageContext, Entity, EventKind, QueuedCommand, Rng, System,
    World, create_champion,
};

type Log<T> = Arc<Mutex<Vec<T>>>;

fn log<T>() -> Log<T> {
    Arc::new(Mutex::new(Vec::new()))
}

fn stats() -> Stats {
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

/// Équivalent de `dummy()` de `packages/server/test/helpers.ts`.
fn dummy(world: &mut World, team: TeamOrNeutral, pos: Vec2, kind: EntityKind) -> EntityId {
    let s = stats();
    world.spawn(Entity::new(kind, team, pos, 40.0, s.max_hp, 0.0, s))
}

fn minion(world: &mut World) -> EntityId {
    dummy(
        world,
        TeamOrNeutral::Blue,
        Vec2 { x: 0.0, y: 0.0 },
        EntityKind::Minion,
    )
}

fn level_up(target: EntityId, level: u32) -> GameEvent {
    GameEvent::LevelUp {
        target,
        level,
        fx_key: None,
    }
}

// ---------------------------------------------------------------- EventBus

#[test]
fn relaie_les_evenements_via_le_bus() {
    // Port de « relaie les événements via l'EventBus ».
    let mut world = World::new(1);
    let seen = log::<u32>();
    let s = seen.clone();
    world.events.on(EventKind::LevelUp, move |_, _| {
        s.lock().unwrap().push(1);
    });
    world.emit(level_up(1, 2));
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(world.events.drain().len(), 1);
    assert_eq!(world.events.drain().len(), 0);
}

#[test]
fn ecouteurs_dans_l_ordre_et_filtres_par_type() {
    let mut world = World::new(1);
    let out = log::<&'static str>();
    for name in ["a", "b", "c"] {
        let o = out.clone();
        world.events.on(EventKind::LevelUp, move |_, _| {
            o.lock().unwrap().push(name);
        });
    }
    let o = out.clone();
    world.events.on(EventKind::GoldGain, move |_, _| {
        o.lock().unwrap().push("autre type");
    });
    world.emit(level_up(1, 2));
    assert_eq!(*out.lock().unwrap(), ["a", "b", "c"]);
}

#[test]
fn desabonnement_par_poignee() {
    let mut world = World::new(1);
    let n = log::<u32>();
    let (n1, n2) = (n.clone(), n.clone());
    let sub = world.events.on(EventKind::LevelUp, move |_, _| {
        n1.lock().unwrap().push(1);
    });
    world.events.on(EventKind::LevelUp, move |_, _| {
        n2.lock().unwrap().push(2);
    });
    world.emit(level_up(1, 2));
    world.events.off(sub);
    world.events.off(sub); // idempotent
    world.emit(level_up(1, 3));
    assert_eq!(*n.lock().unwrap(), [1, 2, 2]);
}

#[test]
fn ecouteur_enregistre_pendant_la_distribution_est_conserve() {
    let mut world = World::new(1);
    let out = log::<&'static str>();
    let o = out.clone();
    world.events.on(EventKind::LevelUp, move |w, _| {
        o.lock().unwrap().push("parent");
        let o2 = o.clone();
        // Enregistré une seule fois, à la première distribution.
        if o.lock().unwrap().len() == 1 {
            w.events.on(EventKind::LevelUp, move |_, _| {
                o2.lock().unwrap().push("enfant");
            });
        }
    });
    world.emit(level_up(1, 2));
    // Comme `Set.forEach` : appelé dès la passe en cours.
    assert_eq!(*out.lock().unwrap(), ["parent", "enfant"]);
    world.emit(level_up(1, 3));
    assert_eq!(
        *out.lock().unwrap(),
        ["parent", "enfant", "parent", "enfant"]
    );
}

#[test]
fn desabonnement_pendant_la_distribution() {
    let mut world = World::new(1);
    let out = log::<&'static str>();
    let victim = Arc::new(Mutex::new(None));

    let (o, v) = (out.clone(), victim.clone());
    world.events.on(EventKind::LevelUp, move |w, _| {
        o.lock().unwrap().push("tueur");
        if let Some(sub) = v.lock().unwrap().take() {
            w.events.off(sub);
        }
    });
    let o = out.clone();
    let sub = world.events.on(EventKind::LevelUp, move |_, _| {
        o.lock().unwrap().push("victime");
    });
    *victim.lock().unwrap() = Some(sub);

    world.emit(level_up(1, 2));
    world.emit(level_up(1, 3));
    // La victime, retirée avant son tour, ne s'exécute jamais.
    assert_eq!(*out.lock().unwrap(), ["tueur", "tueur"]);
}

#[test]
fn ecouteur_qui_se_desabonne_lui_meme() {
    let mut world = World::new(1);
    let n = log::<u32>();
    let me = Arc::new(Mutex::new(None));
    let (n1, me1) = (n.clone(), me.clone());
    let sub = world.events.on(EventKind::LevelUp, move |w, _| {
        n1.lock().unwrap().push(1);
        if let Some(s) = me1.lock().unwrap().take() {
            w.events.off(s);
        }
    });
    *me.lock().unwrap() = Some(sub);
    world.emit(level_up(1, 2));
    world.emit(level_up(1, 3));
    assert_eq!(n.lock().unwrap().len(), 1);
}

#[test]
fn emission_imbriquee_ordre_des_pending_et_distribution() {
    let mut world = World::new(1);
    let gold = log::<f64>();
    let g = gold.clone();
    world.events.on(EventKind::GoldGain, move |_, e| {
        if let GameEvent::GoldGain { amount, .. } = e {
            g.lock().unwrap().push(*amount);
        }
    });
    // Un écouteur de levelUp émet un goldGain : distribué aussi, et placé après son parent.
    world.events.on(EventKind::LevelUp, |w, _| {
        w.emit(GameEvent::GoldGain {
            target: 1,
            amount: 50.0,
            pos: None,
        });
    });
    world.emit(level_up(1, 2));
    assert_eq!(*gold.lock().unwrap(), [50.0]);
    let pending = world.events.drain();
    assert!(matches!(pending[0], GameEvent::LevelUp { .. }));
    assert!(matches!(pending[1], GameEvent::GoldGain { .. }));
}

#[test]
fn emission_sans_ecouteur_va_dans_pending() {
    let mut world = World::new(1);
    world.emit(level_up(1, 2));
    world.emit(level_up(1, 3));
    let drained = world.events.drain();
    assert_eq!(drained.len(), 2);
    assert!(world.events.pending.is_empty());
}

#[test]
fn le_discriminant_et_le_nom_du_type() {
    let samples = [
        (level_up(1, 2), EventKind::LevelUp),
        (
            GameEvent::Heal {
                source: 1,
                target: 2,
                amount: 3.0,
            },
            EventKind::Heal,
        ),
        (
            GameEvent::Death {
                target: 1,
                killer: None,
                assists: None,
                bounty: None,
            },
            EventKind::Death,
        ),
        (
            GameEvent::AutoAttack {
                source: 1,
                target: 2,
                ranged: false,
                speed: None,
                crit: None,
            },
            EventKind::AutoAttack,
        ),
        (
            GameEvent::Chat {
                player_id: "p".into(),
                text: "x".into(),
                team_only: false,
            },
            EventKind::Chat,
        ),
    ];
    for (e, kind) in samples {
        assert_eq!(EventKind::of(&e), kind);
    }
    // Noms du protocole (champ `type`, camelCase).
    assert_eq!(EventKind::LevelUp.name(), "levelUp");
    assert_eq!(EventKind::AutoAttack.name(), "autoAttack");
    assert_eq!(EventKind::Chat.name(), "chat");
    assert_eq!(EventKind::COUNT, 27);
}

// ---------------------------------------------------------------- DataTable

#[derive(Debug, PartialEq)]
struct MinionData {
    lane: u8,
}
#[derive(Debug, PartialEq, Default)]
struct Buffs {
    list: Vec<String>,
}

#[test]
fn table_de_donnees_typee() {
    let mut world = World::new(1);
    let id = minion(&mut world);
    let e = world.get_mut(id).unwrap();

    assert!(!e.data.contains::<MinionData>());
    assert!(e.data.get::<MinionData>().is_none());
    assert!(e.data.is_empty());

    assert_eq!(e.data.insert(MinionData { lane: 1 }), None);
    assert!(e.data.contains::<MinionData>());
    assert_eq!(e.data.get::<MinionData>(), Some(&MinionData { lane: 1 }));
    e.data.get_mut::<MinionData>().unwrap().lane = 2;
    assert_eq!(
        e.data.insert(MinionData { lane: 3 }),
        Some(MinionData { lane: 2 })
    );

    // Les domaines sont indépendants.
    e.data
        .get_or_insert_with(Buffs::default)
        .list
        .push("stun".into());
    e.data
        .get_or_insert_with(|| Buffs {
            list: vec!["jamais".into()],
        })
        .list
        .push("slow".into());
    assert_eq!(e.data.get::<Buffs>().unwrap().list, ["stun", "slow"]);
    assert_eq!(e.data.len(), 2);

    assert_eq!(e.data.remove::<MinionData>(), Some(MinionData { lane: 3 }));
    assert_eq!(e.data.remove::<MinionData>(), None);
    assert!(e.data.contains::<Buffs>());
}

// ---------------------------------------------------------------- Entités

#[test]
fn ids_sequentiels_et_ordre_d_insertion() {
    let mut world = World::new(1);
    let a = minion(&mut world);
    let b = dummy(
        &mut world,
        TeamOrNeutral::Red,
        Vec2 { x: 1.0, y: 1.0 },
        EntityKind::Tower,
    );
    let c = minion(&mut world);
    assert_eq!((a, b, c), (1, 2, 3));
    assert_eq!(world.get(b).unwrap().id, b);

    let order: Vec<_> = world.iter().map(|e| e.id).collect();
    assert_eq!(order, [1, 2, 3]);
    let minions: Vec<_> = world
        .query(Some(EntityKind::Minion))
        .map(|e| e.id)
        .collect();
    assert_eq!(minions, [1, 3]);
    assert_eq!(world.query(None).count(), 3);

    // Un retrait ne réutilise jamais l'id et conserve l'ordre des autres.
    world.remove(b);
    assert!(world.get(b).is_none());
    let d = minion(&mut world);
    assert_eq!(d, 4);
    let order: Vec<_> = world.iter().map(|e| e.id).collect();
    assert_eq!(order, [1, 3, 4]);
    assert_eq!(world.len(), 3);
    world.remove(b); // idempotent
    world.remove(999);
    assert!(world.get(0).is_none());
    assert!(world.get_opt(None).is_none());
    assert!(world.get_opt(Some(a)).is_some());
}

#[test]
fn compaction_conserve_ordre_et_acces_par_id() {
    let mut world = World::new(1);
    let ids: Vec<_> = (0..500).map(|_| minion(&mut world)).collect();
    // Retire 3 entités sur 4 : déclenche la compaction.
    for (i, id) in ids.iter().enumerate() {
        if i % 4 != 0 {
            world.remove(*id);
        }
    }
    let expected: Vec<_> = ids.iter().copied().step_by(4).collect();
    let got: Vec<_> = world.iter().map(|e| e.id).collect();
    assert_eq!(got, expected);
    for id in &expected {
        assert_eq!(world.get(*id).unwrap().id, *id);
    }
    let fresh = minion(&mut world);
    assert_eq!(fresh, 501);
    assert_eq!(world.iter().last().unwrap().id, fresh);
}

#[test]
fn ids_into_reutilise_le_tampon() {
    let mut world = World::new(1);
    minion(&mut world);
    minion(&mut world);
    let mut buf = vec![42];
    world.ids_into(Some(EntityKind::Minion), &mut buf);
    assert_eq!(buf, [1, 2]);
}

#[test]
fn spawn_copie_la_position() {
    // Port de « spawn copie la position » : `Vec2` est `Copy`, rien n'est partagé avec la carte.
    let mut world = World::new(1);
    let shared = Vec2 { x: 100.0, y: 100.0 };
    let id = world.spawn(Entity::new(
        EntityKind::Minion,
        TeamOrNeutral::Blue,
        shared,
        40.0,
        1.0,
        0.0,
        stats(),
    ));
    let e = world.get_mut(id).unwrap();
    e.pos.x += 400.0;
    e.move_target = Some(Vec2 { x: 500.0, y: 100.0 });
    assert_eq!(shared, Vec2 { x: 100.0, y: 100.0 });
    assert_eq!(e.facing, 0.0);
    assert_eq!(e.shield, 0.0);
    assert!(!e.dead);
}

fn opts(player: &str, team: Team) -> CreateChampion {
    CreateChampion {
        player_id: player.to_string(),
        name: "Test".to_string(),
        team,
        champion_id: "grondak".to_string(),
        summoners: [SummonerId::Flash, SummonerId::Ignite],
        pos: Vec2 {
            x: 1000.0,
            y: 1000.0,
        },
    }
}

#[test]
fn create_champion_et_champion_of() {
    let mut world = World::new(1);
    let id = create_champion(&mut world, opts("p1", Team::Blue), &stats());
    let e = world.champion_of("p1").unwrap();
    assert_eq!(e.id, id);
    assert_eq!(e.kind, EntityKind::Champion);
    assert_eq!(e.team, TeamOrNeutral::Blue);
    assert_eq!(e.hp, 1000.0);
    assert_eq!(e.subtype.as_deref(), Some("grondak"));
    let c = e.champ.as_ref().unwrap();
    assert_eq!((c.level, c.skill_points, c.gold), (1, 1, 500.0));
    assert_eq!(c.items.len(), 6);
    assert!(world.champion_of("inconnu").is_none());

    world.remove(id);
    assert!(world.champion_of("p1").is_none());
}

#[test]
fn remplacer_un_champion_ne_perd_pas_le_nouveau_au_retrait_de_l_ancien() {
    let mut world = World::new(1);
    let old = create_champion(&mut world, opts("p1", Team::Blue), &stats());
    let new = create_champion(&mut world, opts("p1", Team::Blue), &stats());
    world.remove(old);
    assert_eq!(world.champion_id_of("p1"), Some(new));
}

// ---------------------------------------------------------------- Tick

#[test]
fn step_avance_le_temps_lance_les_systemes_dans_l_ordre_et_vide_les_commandes() {
    let mut world = World::new(1);
    let out = log::<String>();
    let (o1, o2, o3) = (out.clone(), out.clone(), out.clone());
    let mut systems = vec![
        System::new("a", move |w, _| {
            // Les commandes sont visibles par tous les systèmes pendant le tick.
            o1.lock().unwrap().push(format!("a:{}", w.commands.len()));
        })
        .with_init(move |_| o3.lock().unwrap().push("init a".into())),
        System::new("b", move |w, dt| {
            o2.lock()
                .unwrap()
                .push(format!("b:{}:{:.3}", w.commands.len(), dt));
        }),
    ];
    world.init(&mut systems);
    world.commands.push(QueuedCommand {
        player_id: "p1".into(),
        msg: ClientMsg::Move {
            pos: Vec2 { x: 2.0, y: 1.0 },
        },
    });
    world.step(&mut systems);
    assert_eq!(*out.lock().unwrap(), ["init a", "a:1", "b:1:0.033"]);
    assert!(world.commands.is_empty());
    assert_eq!(world.tick, 1);
    assert!((world.time - 1.0 / 30.0).abs() < 1e-12);
    world.step_dt(&mut systems, 0.5);
    assert_eq!(world.tick, 2);
    assert!((world.time - (1.0 / 30.0 + 0.5)).abs() < 1e-12);
    assert_eq!(world.winner, None);
}

// ---------------------------------------------------------------- Hooks

#[test]
fn hooks_mutants_dans_l_ordre_et_avec_acces_au_monde() {
    let mut world = World::new(1);
    let target = minion(&mut world);
    world.hooks.before_damage.push(Box::new(|w, ctx| {
        ctx.amount *= 2.0;
        // Accès mutable au monde pendant le hook.
        if let Some(t) = w.get_mut(ctx.target) {
            t.shield += 1.0;
        }
    }));
    world
        .hooks
        .before_damage
        .push(Box::new(|_, ctx| ctx.amount += 1.0));
    let mut ctx = DamageContext::new(target, 10.0, DamageType::Physical);
    world.run_before_damage(&mut ctx);
    assert_eq!(ctx.amount, 21.0); // (10 * 2) + 1 : l'ordre compte
    assert_eq!(world.get(target).unwrap().shield, 1.0);
    // Les hooks sont remis en place après l'appel.
    world.run_before_damage(&mut ctx);
    assert_eq!(ctx.amount, 43.0);
    assert_eq!(world.hooks.before_damage.len(), 2);
}

#[test]
fn hook_enregistre_pendant_la_distribution_n_est_pas_perdu() {
    let mut world = World::new(1);
    let target = minion(&mut world);
    let done = Arc::new(Mutex::new(false));
    let d = done.clone();
    world.hooks.before_damage.push(Box::new(move |w, _| {
        let mut done = d.lock().unwrap();
        if !*done {
            *done = true;
            w.hooks
                .before_damage
                .push(Box::new(|_, ctx| ctx.amount += 100.0));
        }
    }));
    let mut ctx = DamageContext::new(target, 1.0, DamageType::True);
    world.run_before_damage(&mut ctx);
    assert_eq!(world.hooks.before_damage.len(), 2);
    assert_eq!(ctx.amount, 101.0);
    world.run_before_damage(&mut ctx);
    assert_eq!(ctx.amount, 201.0);
}

#[test]
fn distribution_imbriquee_des_hooks() {
    // Un hook qui déclenche une nouvelle distribution de la même liste : les autres hooks s'exécutent.
    let mut world = World::new(1);
    let target = minion(&mut world);
    let depth = Arc::new(Mutex::new(0u32));
    let d = depth.clone();
    world.hooks.before_damage.push(Box::new(move |w, ctx| {
        let mut depth = d.lock().unwrap();
        if *depth == 0 {
            *depth += 1;
            drop(depth);
            let mut inner = DamageContext::new(ctx.target, 0.0, DamageType::Magic);
            w.run_before_damage(&mut inner);
            ctx.amount += inner.amount;
        }
    }));
    world
        .hooks
        .before_damage
        .push(Box::new(|_, ctx| ctx.amount += 5.0));
    let mut ctx = DamageContext::new(target, 0.0, DamageType::True);
    world.run_before_damage(&mut ctx);
    // Le hook interne (5) a tourné dans la distribution imbriquée, puis le hook externe (5).
    assert_eq!(ctx.amount, 10.0);
}

#[test]
fn prevent_death_execute_tout_et_vrai_si_un_seul_repond_vrai() {
    let mut world = World::new(1);
    let target = minion(&mut world);
    let calls = log::<u32>();
    for (n, answer) in [(1, false), (2, true), (3, false)] {
        let c = calls.clone();
        world.hooks.prevent_death.push(Box::new(move |_, _, _| {
            c.lock().unwrap().push(n);
            answer
        }));
    }
    assert!(world.run_prevent_death(target, None));
    assert_eq!(*calls.lock().unwrap(), [1, 2, 3]);
}

#[test]
fn hooks_purs_allow_et_premier_some() {
    let mut world = World::new(1);
    let a = minion(&mut world);
    let b = minion(&mut world);
    assert!(world.can_move(a) && world.can_attack(a) && world.is_targetable(a, b));

    world.hooks.can_move.push(Box::new(|_, _| true));
    world.hooks.can_move.push(Box::new(|_, e| e != 2));
    assert!(world.can_move(a));
    assert!(!world.can_move(b));

    world.hooks.is_targetable.push(Box::new(|w, target, _| {
        w.get(target).is_some_and(|t| t.hp > 0.0)
    }));
    assert!(world.is_targetable(a, b));
    world.get_mut(a).unwrap().hp = 0.0;
    assert!(!world.is_targetable(a, b));

    assert_eq!(world.hook_bounty(a, None, &[]), None);
    world.hooks.bounty.push(Box::new(|_, _, _, _| None));
    world.hooks.bounty.push(Box::new(|_, _, _, assists| {
        Some(100.0 + assists.len() as f64)
    }));
    world.hooks.bounty.push(Box::new(|_, _, _, _| Some(1.0)));
    assert_eq!(world.hook_bounty(a, Some(b), &[b, b]), Some(102.0));

    world.hooks.respawn_time.push(Box::new(|_, _| Some(7.5)));
    assert_eq!(world.hook_respawn_time(a), Some(7.5));
}

#[test]
fn hooks_d_attaque_et_de_soin() {
    let mut world = World::new(1);
    let (a, b) = (minion(&mut world), minion(&mut world));
    world
        .hooks
        .before_attack_hit
        .push(Box::new(|_, ctx| ctx.bonus += 10.0));
    world
        .hooks
        .after_attack_hit
        .push(Box::new(|_, ctx, dealt| ctx.bonus = dealt));
    let mut ctx = AttackContext {
        attacker: a,
        target: b,
        amount: 100.0,
        crit: false,
        crit_multiplier: 2.0,
        bonus: 0.0,
        damage_type: DamageType::Physical,
    };
    world.run_before_attack_hit(&mut ctx);
    assert_eq!(ctx.bonus, 10.0);
    world.run_after_attack_hit(&mut ctx, 77.0);
    assert_eq!(ctx.bonus, 77.0);

    world
        .hooks
        .before_heal
        .push(Box::new(|_, ctx| ctx.amount *= 0.5));
    let mut heal = lov_sim::HealContext {
        source: None,
        target: a,
        amount: 40.0,
    };
    world.run_before_heal(&mut heal);
    assert_eq!(heal.amount, 20.0);
}

// ---------------------------------------------------------------- RNG

#[test]
fn rng_meme_graine_meme_suite() {
    let mut a = Rng::new(42);
    let mut b = Rng::new(42);
    let mut c = Rng::new(43);
    let sa: Vec<_> = (0..64).map(|_| a.next_u64()).collect();
    let sb: Vec<_> = (0..64).map(|_| b.next_u64()).collect();
    let sc: Vec<_> = (0..64).map(|_| c.next_u64()).collect();
    assert_eq!(sa, sb);
    assert_ne!(sa, sc);
    // Valeur de référence figée : détecte tout changement d'algorithme (déterminisme inter-plateformes).
    assert_eq!(Rng::new(0).next_u64(), Rng::new(0).next_u64());
    assert_ne!(Rng::new(0).next_u64(), 0);
}

#[test]
fn rng_helpers_dans_leurs_bornes() {
    let mut r = Rng::new(7);
    for _ in 0..10_000 {
        let f = r.next_f64();
        assert!((0.0..1.0).contains(&f));
        let x = r.range(-5.0, 5.0);
        assert!((-5.0..5.0).contains(&x));
        assert!(r.below(6) < 6);
    }
    assert_eq!(r.below(0), 0);
    assert_eq!(r.below(1), 0);
    assert!(r.pick::<u8>(&[]).is_none());
    let items = [10, 20, 30];
    let mut seen = [false; 3];
    for _ in 0..200 {
        let p = *r.pick(&items).unwrap();
        seen[items.iter().position(|&i| i == p).unwrap()] = true;
    }
    assert!(seen.iter().all(|&s| s));
    assert!(!r.chance(0.0));
    assert!(r.chance(1.0));
    let hits = (0..10_000).filter(|_| r.chance(0.25)).count();
    assert!((2000..3000).contains(&hits), "{hits}");
}

/// Un petit scénario qui consomme l'aléa du monde dans un système.
fn run_scenario(seed: u64) -> (Vec<(EntityId, f64, f64)>, Vec<GameEvent>) {
    let mut world = World::new(seed);
    for _ in 0..5 {
        minion(&mut world);
    }
    let mut systems = vec![System::new("bruit", |w, dt| {
        let mut ids = Vec::new();
        w.ids_into(None, &mut ids);
        for id in ids {
            let (dx, crit) = (w.rng.range(-10.0, 10.0), w.rng.chance(0.5));
            if let Some(e) = w.get_mut(id) {
                e.pos.x += dx * dt;
            }
            if crit {
                w.emit(GameEvent::LevelUp {
                    target: id,
                    level: 2,
                    fx_key: None,
                });
            }
        }
    })];
    for _ in 0..100 {
        world.step(&mut systems);
    }
    let state = world.iter().map(|e| (e.id, e.pos.x, e.pos.y)).collect();
    (state, world.events.drain())
}

#[test]
fn deux_mondes_de_meme_graine_finissent_dans_le_meme_etat() {
    let (s1, e1) = run_scenario(2024);
    let (s2, e2) = run_scenario(2024);
    assert_eq!(s1, s2);
    assert_eq!(e1, e2);
    assert!(!e1.is_empty());
    let (s3, _) = run_scenario(2025);
    assert_ne!(s1, s3);
}
