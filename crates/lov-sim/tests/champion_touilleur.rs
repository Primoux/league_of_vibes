//! Kit du Touilleur : port de `packages/server/test/champion-touilleur.test.ts` (cible `tharox` au lieu
//! de `sable` ; les `it.each` sont portés en boucles).

mod common;

use common::champion::{Game, game};
use lov_shared::data::champions::get_champion;
use lov_shared::map::WALLS;
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::{AbilityParam, CcType};
use lov_shared::types::{AbilitySlot, DamageType, GameEvent, TeamOrNeutral, Vec2};
use lov_sim::QueuedCommand;
use lov_sim::abilities::primitives::is_walkable;
use lov_sim::combat::buffs::{
    BuffOptions, Channeling, Stasis, add_buff, can_attack, can_cast, can_move, cleanse, get_buff,
    has_cc,
};
use lov_sim::combat::damage::{DamageOpts, apply_damage};

fn p(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

/// `ultimate(facing)` : R lancé sur la cible orientée vers `facing`, 0,26 s plus tard.
fn ultimate(facing: f64) -> Game {
    let mut g = game("touilleur");
    let t = g.target;
    g.e_mut(t).facing = facing;
    assert!(g.cast(AbilitySlot::R, Some(t), None));
    g.advance(0.26);
    g
}

fn channeling(g: &Game) -> bool {
    g.e(g.source).data.contains::<Channeling>()
}

fn my_stun(g: &Game) -> bool {
    get_buff(g.e(g.target), "stun", Some(g.source)).is_some()
}

fn spell(g: &mut Game, target: u32, amount: f64) {
    let mut opts = DamageOpts::new(Some(g.source), target, amount, DamageType::True);
    opts.ability = Some("touilleur_q".into());
    apply_damage(&mut g.world, opts);
}

#[test]
fn kit_jouable_et_ultime_de_deux_secondes() {
    let def = get_champion("touilleur").unwrap();
    assert_eq!(def.name, "Le Touilleur");
    let duration = def
        .abilities
        .r
        .params
        .as_ref()
        .and_then(|p| p.get("duration"));
    assert_eq!(duration, Some(&AbilityParam::One(2.0)));
}

#[test]
fn q_touche_et_ralentit_uniquement_le_premier_ennemi() {
    let mut g = game("touilleur");
    let t = g.target;
    let behind = g.dummy(p(1500.0, 1000.0), |_| {});
    g.cast(AbilitySlot::Q, Some(t), None);
    g.advance(0.5);
    assert!(g.damage(Some("touilleur_q"), Some(t)) > 0.0);
    assert!(has_cc(g.e(t), CcType::Slow));
    assert_eq!(g.damage(Some("touilleur_q"), Some(behind)), 0.0);
}

#[test]
fn w_respecte_sa_telegraphie_et_epargne_allies_et_cibles_hors_zone() {
    let mut g = game("touilleur");
    let t = g.target;
    let tpos = g.e(t).pos;
    let ally = g.dummy(tpos, |_| {});
    g.e_mut(ally).team = TeamOrNeutral::Blue;
    let far = g.dummy(p(1800.0, 1000.0), |_| {});
    g.cast(AbilitySlot::W, Some(t), None);
    g.advance(0.5);
    assert_eq!(g.damage(Some("touilleur_w"), None), 0.0);
    g.advance(0.15);
    assert!(g.damage(Some("touilleur_w"), Some(t)) > 0.0);
    assert!(has_cc(g.e(t), CcType::Slow));
    assert_eq!(g.damage(Some("touilleur_w"), Some(ally)), 0.0);
    assert_eq!(g.damage(Some("touilleur_w"), Some(far)), 0.0);
}

#[test]
fn e_borne_son_deplacement_et_son_bouclier_expire() {
    let mut g = game("touilleur");
    let s = g.source;
    g.cast(AbilitySlot::E, None, Some(p(1700.0, 1000.0)));
    assert!((g.e(s).pos.x - 1325.0).abs() < 1.0);
    assert_eq!(g.e(s).shield, 45.0);
    g.advance(2.1);
    assert_eq!(g.e(s).shield, 0.0);
}

#[test]
fn immobilisation_et_cloue_au_sol_empechent_e_et_r_sans_cout() {
    for control in ["root", "grounded"] {
        let mut g = game("touilleur");
        let (s, t) = (g.source, g.target);
        let mana = g.e(s).mana;
        add_buff(&mut g.world, s, control, BuffOptions::new(1.0));
        assert!(!g.cast(AbilitySlot::E, None, None), "{control}");
        assert!(!g.cast(AbilitySlot::R, Some(t), None), "{control}");
        assert_eq!(g.e(s).mana, mana);
        assert_eq!(g.e(s).champ.as_ref().unwrap().cooldowns.r, 0.0);
    }
}

#[test]
fn r_arrive_dans_le_dos_de_la_cible_quelle_que_soit_son_orientation() {
    use std::f64::consts::PI;
    for facing in [0.0, PI / 2.0, PI, -PI / 2.0] {
        let g = ultimate(facing);
        let (s, t) = (g.e(g.source), g.e(g.target));
        let (dx, dy) = (s.pos.x - t.pos.x, s.pos.y - t.pos.y);
        assert!(
            dx * facing.cos() + dy * facing.sin() < -t.radius,
            "{facing}"
        );
        assert!(dx.hypot(dy) >= s.radius + t.radius);
        assert!(is_walkable(s.pos, s.radius));
        assert!(g.damage(Some("touilleur_r"), Some(g.target)) > 0.0);
    }
}

#[test]
fn r_rejoint_le_dos_meme_a_la_limite_de_portee() {
    let mut g = game("touilleur");
    let (s, t) = (g.source, g.target);
    let (sr, tr, sx) = (g.e(s).radius, g.e(t).radius, g.e(s).pos.x);
    g.e_mut(t).pos.x = sx + 450.0 + sr + tr;
    g.e_mut(t).facing = std::f64::consts::PI;
    assert!(g.cast(AbilitySlot::R, Some(t), None));
    g.advance(0.26);
    assert!(g.e(s).pos.x > g.e(t).pos.x);
    assert!(has_cc(g.e(t), CcType::Stun));
}

#[test]
fn r_etourdit_deux_secondes_et_bloque_le_lanceur() {
    let mut g = ultimate(0.0);
    let (s, t) = (g.source, g.target);
    assert_eq!(
        get_buff(g.e(t), "stun", Some(s)).map(|b| b.duration),
        Some(2.0)
    );
    for id in [t, s] {
        let e = g.e(id);
        assert!(!can_move(e) && !can_attack(e) && !can_cast(e));
    }
    g.advance(1.9);
    assert!(has_cc(g.e(t), CcType::Stun));
    assert!(channeling(&g));
    g.advance(0.11);
    assert!(!has_cc(g.e(t), CcType::Stun));
    assert!(!channeling(&g));
    assert!(can_move(g.e(s)));
}

#[test]
fn la_tenacite_reduit_l_etourdissement_et_la_canalisation() {
    let mut g = game("touilleur");
    let (s, t) = (g.source, g.target);
    let mut tenacious = BuffOptions::new(10.0);
    tenacious.params.tenacity = Some(0.5);
    add_buff(&mut g.world, t, "tenacious", tenacious);
    g.cast(AbilitySlot::R, Some(t), None);
    g.advance(0.26);
    assert_eq!(
        get_buff(g.e(t), "stun", Some(s)).map(|b| b.duration),
        Some(1.0)
    );
    g.advance(1.01);
    assert!(!has_cc(g.e(t), CcType::Stun));
    assert!(!channeling(&g));
}

#[test]
fn la_purification_termine_l_etourdissement_et_la_touille() {
    let mut g = ultimate(0.0);
    let (s, t) = (g.source, g.target);
    cleanse(&mut g.world, t);
    assert!(!has_cc(g.e(t), CcType::Stun));
    assert!(!channeling(&g));
    g.advance(3.0);
    assert!(!has_cc(g.e(t), CcType::Stun));
    assert!(get_buff(g.e(s), "touilleur_r_channel", None).is_none());
}

#[test]
fn un_controle_du_lanceur_interrompt_r_sans_retirer_ceux_d_une_autre_source() {
    for control in ["stun", "silence", "knockup"] {
        let mut g = ultimate(0.0);
        let (s, t) = (g.source, g.target);
        let other = g.dummy(p(1400.0, 1000.0), |_| {});
        let mut theirs = BuffOptions::new(3.0);
        theirs.source = Some(other);
        add_buff(&mut g.world, t, "stun", theirs);
        add_buff(&mut g.world, s, control, BuffOptions::new(1.0));
        g.advance(0.04);
        assert!(!channeling(&g), "{control}");
        assert!(!my_stun(&g), "{control}");
        assert!(get_buff(g.e(t), "stun", Some(other)).is_some(), "{control}");
    }
}

#[test]
fn la_mort_d_un_des_deux_arrete_r_sans_verrou_residuel() {
    for side in ["source", "target"] {
        let mut g = ultimate(0.0);
        let (s, t) = (g.source, g.target);
        let (victim, killer) = if side == "source" { (s, t) } else { (t, s) };
        apply_damage(
            &mut g.world,
            DamageOpts::new(Some(killer), victim, 1e6, DamageType::True),
        );
        assert!(!channeling(&g), "{side}");
        assert!(!my_stun(&g), "{side}");
        g.advance(3.0);
        assert!(!channeling(&g), "{side}");
    }
}

#[test]
fn un_deplacement_force_de_la_cible_rompt_la_touille() {
    let mut g = ultimate(0.0);
    let t = g.target;
    g.e_mut(t).pos.x += 500.0;
    g.advance(0.04);
    assert!(!channeling(&g));
    assert!(!has_cc(g.e(t), CcType::Stun));
}

#[test]
fn la_stase_d_un_des_deux_rompt_la_touille() {
    for side in ["source", "target"] {
        let mut g = ultimate(0.0);
        let id = if side == "source" { g.source } else { g.target };
        g.e_mut(id).data.insert(Stasis);
        g.advance(0.04);
        assert!(!channeling(&g), "{side}");
        assert!(!my_stun(&g), "{side}");
    }
}

#[test]
fn une_cible_retiree_libere_le_lanceur() {
    let mut g = ultimate(0.0);
    let (s, t) = (g.source, g.target);
    g.world.remove(t);
    g.advance(0.04);
    assert!(!channeling(&g));
    assert!(can_move(g.e(s)));
}

#[test]
fn r_n_arrive_ni_dans_un_mur_ni_devant_sa_cible() {
    let mut g = game("touilleur");
    let (s, t) = (g.source, g.target);
    let wall = &WALLS[0];
    let edge = wall.iter().map(|q| q.x).fold(f64::INFINITY, f64::min);
    let y = wall.iter().map(|q| q.y).sum::<f64>() / wall.len() as f64;
    let tr = g.e(t).radius;
    g.e_mut(t).pos = p(edge - tr - 2.0, y);
    g.e_mut(t).facing = std::f64::consts::PI;
    let tx = g.e(t).pos.x;
    g.e_mut(s).pos = p(tx - 250.0, y);
    assert!(is_walkable(g.e(s).pos, g.e(s).radius));
    assert!(is_walkable(g.e(t).pos, tr));
    g.cast(AbilitySlot::R, Some(t), None);
    g.advance(0.26);
    assert!(is_walkable(g.e(s).pos, g.e(s).radius));
    if has_cc(g.e(t), CcType::Stun) {
        let (sx, tx, f) = (g.e(s).pos.x, g.e(t).pos.x, g.e(t).facing);
        assert!((sx - tx) * f.cos() < -tr * 0.5);
    } else {
        assert_eq!(g.damage(Some("touilleur_r"), None), 0.0);
        assert!(!channeling(&g));
    }
}

#[test]
fn un_dos_colle_au_bord_de_la_carte_n_etourdit_pas_depuis_l_avant() {
    let mut g = game("touilleur");
    let (s, t) = (g.source, g.target);
    let tr = g.e(t).radius;
    g.e_mut(t).pos = p(tr, 1000.0);
    g.e_mut(t).facing = 0.0;
    g.e_mut(s).pos = p(350.0, 1000.0);
    g.cast(AbilitySlot::R, Some(t), None);
    g.advance(0.26);
    assert_eq!(g.e(s).pos.x, 350.0);
    assert!(!has_cc(g.e(t), CcType::Stun));
    assert_eq!(g.damage(Some("touilleur_r"), None), 0.0);
}

#[test]
fn un_ordre_stop_annule_la_canalisation() {
    let mut g = ultimate(0.0);
    let t = g.target;
    let player_id = g.e(g.source).champ.as_ref().unwrap().player_id.clone();
    g.world.commands.push(QueuedCommand {
        player_id,
        msg: ClientMsg::Stop,
    });
    g.advance(0.04);
    assert!(!channeling(&g));
    assert!(!has_cc(g.e(t), CcType::Stun));
}

#[test]
fn r_refuse_sbires_allies_et_cibles_invisibles_sans_cout() {
    let mut g = game("touilleur");
    let (s, t) = (g.source, g.target);
    let tpos = g.e(t).pos;
    let minion = g.dummy(tpos, |_| {});
    let mana = g.e(s).mana;
    assert!(!g.cast(AbilitySlot::R, Some(minion), None));
    g.e_mut(t).team = TeamOrNeutral::Blue;
    assert!(!g.cast(AbilitySlot::R, Some(t), None));
    g.e_mut(t).team = TeamOrNeutral::Red;
    add_buff(&mut g.world, t, "invisible", BuffOptions::new(3.0));
    assert!(!g.cast(AbilitySlot::R, Some(t), None));
    assert_eq!(g.e(s).mana, mana);
    assert_eq!(g.e(s).champ.as_ref().unwrap().cooldowns.r, 0.0);
}

#[test]
fn r_interrompu_avant_son_depart_ne_teleporte_ni_n_etourdit() {
    let mut g = game("touilleur");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::R, Some(t), None);
    add_buff(&mut g.world, s, "stun", BuffOptions::new(1.0));
    g.advance(0.4);
    assert_eq!((g.e(s).pos.x, g.e(s).pos.y), (1000.0, 1000.0));
    assert!(!has_cc(g.e(t), CcType::Stun));
    assert!(!channeling(&g));
}

#[test]
fn le_passif_attend_trois_impacts_espaces_sans_attaques_ni_sbires() {
    let mut g = game("touilleur");
    let (s, t) = (g.source, g.target);
    for _ in 0..8 {
        spell(&mut g, t, 1.0);
    }
    assert_eq!(
        get_buff(g.e(s), "touilleur_count", None).map(|b| b.stacks),
        Some(1)
    );
    g.advance(0.41);
    let mut auto = DamageOpts::new(Some(s), t, 1.0, DamageType::True);
    auto.is_auto_attack = true;
    apply_damage(&mut g.world, auto);
    let minion = g.dummy(p(1400.0, 1000.0), |_| {});
    spell(&mut g, minion, 1.0);
    assert_eq!(
        get_buff(g.e(s), "touilleur_count", None).map(|b| b.stacks),
        Some(1)
    );
    for _ in 0..2 {
        spell(&mut g, t, 1.0);
        g.advance(0.41);
    }
    assert!(g.damage(Some("touilleur_p"), Some(t)) > 0.0);
    assert!(get_buff(g.e(s), "touilleur_count", None).is_none());
    assert!(get_buff(g.e(s), "touilleur_p_speed", None).is_some());
}

#[test]
fn un_proc_passif_deja_letal_ne_cause_pas_de_seconde_mort() {
    let mut g = game("touilleur");
    let t = g.target;
    for _ in 0..2 {
        spell(&mut g, t, 1.0);
        g.advance(0.41);
    }
    g.e_mut(t).hp = 1.0;
    spell(&mut g, t, 2.0);
    assert!(g.e(t).dead);
    assert_eq!(g.damage(Some("touilleur_p"), None), 0.0);
    let deaths = g
        .world
        .events
        .pending
        .iter()
        .filter(|e| matches!(e, GameEvent::Death { target, .. } if *target == t))
        .count();
    assert_eq!(deaths, 1);
}
