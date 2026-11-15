//! Tests du système de vision : port de `packages/server/test/vision.test.ts`.
//!
//! Le monde de test ne contient que le système de vision (`coreWorld()` du TS y ajoute `input`,
//! `movement` et `combat`, inutiles ici : on déplace les entités en modifiant `pos` directement).
//! Un test n'est pas porté : voir le commentaire `NON PORTÉ` en bas de fichier.

use lov_shared::map::BRUSHES;
use lov_shared::types::{
    DamageType, EntityId, EntityKind, Stats, SummonerId, Team, TeamOrNeutral, Vec2,
};
use lov_sim::systems::vision::{Invisible, RevealedUntil, is_visible_to, vision_system};
use lov_sim::{CreateChampion, DamageContext, Entity, System, World, create_champion};

fn v(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

/// Stats de base de grondak (`packages/shared/src/data/champions/grondak.ts`), le champion de `champ()`.
fn stats() -> Stats {
    Stats {
        max_hp: 680.0,
        hp_regen: 0.0,
        max_mana: 0.0,
        mana_regen: 0.0,
        ad: 64.0,
        ap: 0.0,
        armor: 38.0,
        mr: 32.0,
        attack_speed: 0.62,
        attack_range: 150.0,
        move_speed: 335.0,
        crit_chance: 0.0,
        ability_haste: 0.0,
        lifesteal: 0.0,
        armor_pen: 0.0,
        magic_pen: 0.0,
    }
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

struct Sim {
    world: World,
    systems: [System; 1],
    n: u32,
}

impl Sim {
    fn new() -> Self {
        let mut systems = [vision_system()];
        let mut world = World::new(1);
        world.init(&mut systems);
        Self {
            world,
            systems,
            n: 0,
        }
    }

    fn step(&mut self) {
        self.world.step(&mut self.systems);
    }

    /// `run(world, seconds)` du TS : `round(seconds * 30)` ticks.
    fn run(&mut self, seconds: f64) {
        for _ in 0..(seconds * 30.0).round() as u32 {
            self.step();
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
        create_champion(&mut self.world, opts, &stats())
    }

    /// `dummy(world, team, pos, {}, kind)` de `helpers.ts`.
    fn dummy(&mut self, team: TeamOrNeutral, pos: Vec2, kind: EntityKind) -> EntityId {
        let s = base_stats();
        let hp = s.max_hp;
        self.world
            .spawn(Entity::new(kind, team, pos, 40.0, hp, 0.0, s))
    }

    fn visible(&self, id: EntityId, team: TeamOrNeutral) -> bool {
        is_visible_to(self.world.get(id).unwrap(), team, true)
    }

    fn set_pos(&mut self, id: EntityId, pos: Vec2) {
        self.world.get_mut(id).unwrap().pos = pos;
    }
}

fn brush_center(i: usize) -> Vec2 {
    let b = &BRUSHES[i];
    let n = b.len() as f64;
    v(
        b.iter().map(|p| p.x).sum::<f64>() / n,
        b.iter().map(|p| p.y).sum::<f64>() / n,
    )
}

#[test]
fn rayon_de_vision_des_champions() {
    let mut s = Sim::new();
    s.champ(Team::Blue, v(6000.0, 3000.0));
    let near = s.champ(Team::Red, v(6000.0, 4000.0));
    let far = s.champ(Team::Red, v(6000.0, 5000.0));
    s.step();
    assert!(s.visible(near, TeamOrNeutral::Blue));
    assert!(!s.visible(far, TeamOrNeutral::Blue));
    assert!(s.visible(far, TeamOrNeutral::Red));
}

#[test]
fn brousse_cachee_sauf_ennemi_dans_la_meme_brousse_ou_balise() {
    let mut s = Sim::new();
    let c = brush_center(0);
    let hidden = s.champ(Team::Red, c);
    let watcher = s.champ(Team::Blue, v(c.x + 300.0, c.y));
    s.step();
    assert!(!s.visible(hidden, TeamOrNeutral::Blue));
    s.set_pos(watcher, c);
    s.step();
    assert!(s.visible(hidden, TeamOrNeutral::Blue));
    s.set_pos(watcher, v(c.x + 3000.0, c.y));
    s.step();
    assert!(!s.visible(hidden, TeamOrNeutral::Blue));
    s.dummy(TeamOrNeutral::Blue, v(c.x + 400.0, c.y), EntityKind::Ward);
    s.step();
    assert!(s.visible(hidden, TeamOrNeutral::Blue));
}

/// Dans le TS, `applyDamage` (combat, pas encore porté) exécute `hooks.afterDamage` ; ici on exécute ce
/// même hook directement (`World::run_after_damage`) avec le contexte qu'`applyDamage` aurait fourni
/// (source = crédit = `hidden`, cible = `victim`, 10 dégâts bruts). Le comportement testé est bien celui
/// du système de vision (hook + fenêtre de 1 s) ; les PV de la victime ne sont pas modifiés.
#[test]
fn attaquer_depuis_une_brousse_revele_1_s() {
    let mut s = Sim::new();
    let c = brush_center(0);
    let hidden = s.champ(Team::Red, c);
    let victim = s.champ(Team::Blue, v(c.x + 400.0, c.y));
    s.step();
    assert!(!s.visible(hidden, TeamOrNeutral::Blue));
    let mut ctx = DamageContext::new(victim, 10.0, DamageType::True);
    ctx.source = Some(hidden);
    ctx.credit = Some(hidden);
    s.world.run_after_damage(&mut ctx, 10.0);
    s.step();
    assert!(s.visible(hidden, TeamOrNeutral::Blue));
    s.run(1.2);
    assert!(!s.visible(hidden, TeamOrNeutral::Blue));
}

#[test]
fn invisibilite_et_revelation() {
    let mut s = Sim::new();
    s.champ(Team::Blue, v(6000.0, 3000.0));
    let nyx = s.champ(Team::Red, v(6000.0, 3300.0));
    s.world.get_mut(nyx).unwrap().data.insert(Invisible);
    s.step();
    assert!(!s.visible(nyx, TeamOrNeutral::Blue));
    let until = s.world.time + 1.0;
    s.world
        .get_mut(nyx)
        .unwrap()
        .data
        .insert(RevealedUntil(until));
    s.step();
    assert!(s.visible(nyx, TeamOrNeutral::Blue));
}

// NON PORTÉ : « on ne peut pas auto-attaquer une cible invisible ». Ce test exerce le système `combat`
// (auto-attaque, `attack_target`, ciblabilité face à l'invisibilité), pas encore porté ; il arrivera
// avec le portage de `combat`.
