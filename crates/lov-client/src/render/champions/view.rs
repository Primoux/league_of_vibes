//! Vue 3D d'un champion : modèle procédural, animations, mort.
//!
//! Portage de `ProceduralChampionView` (`packages/client/src/render/champions/ChampionView.ts`) et du
//! routage d'événements de `render/champions/index.ts`. Pilotage des animations :
//!  - animation du serveur (`snap.anim`) : idle / run / attack / dash / death / cast_* ;
//!  - événements : autoAttack → frappe, abilityCast → cast_<slot>, damage → recul, recallStart/End ;
//!  - vitesse mesurée à l'écran : cadence de course.
//!
//! La logique est dans `ChampionView::step`, sans ECS (testée) ; `animate_champions` en applique le
//! résultat aux entités.
//!
//! Écarts provisoires : la dissolution de la mort attend le matériau de dissolution du socle FX (#111) —
//! en attendant, après la chute, le contour disparaît puis le modèle d'un coup à la fin du délai ; le
//! crochet de réapparition (FX) attend les FX des champions.

use std::f32::consts::TAU;

use bevy::prelude::*;
use lov_shared::types::{AbilitySlot, EntityId, EntitySnapshot, GameEvent};

use super::anim::clips::generic_clips;
use super::anim::pose::j::*;
use super::anim::pose::{Animator, ClipCtx, DEFAULT_CTX, JOINT_COUNT, POSE_SIZE, Pose};
use super::melenchon::{self, MelenchonView};
use super::model::{ChampionModelDef, SpawnedModel};
use crate::net::ServerEvent;
use crate::render::views::EntityView;

/// Clips pendant lesquels l'arme est simplement portée (le port d'arme s'applique).
const CARRY_CLIPS: [&str; 5] = ["idle", "run", "dash", "stunned", "recall"];
const STUN_BUFFS: [&str; 5] = ["stun", "knockup", "suppress", "taunt", "fear"];
/// Monde/s (≈ 340 u/s).
const NOMINAL_SPEED: f32 = 3.4;

/// Écart angulaire ramené dans [-π, π].
pub fn wrap_angle(a: f32) -> f32 {
    a - TAU * (a / TAU).round()
}

/// Ce que la frame doit écrire dans le monde.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub joints: [Transform; JOINT_COUNT],
    /// Rotation y du modèle (écart d'orientation lissé) et hauteur de projection.
    pub model_yaw: f32,
    pub model_lift: f32,
    pub model_visible: bool,
    pub outlines_visible: bool,
    /// Modèle translucide (invisibilité, visible des alliés seulement).
    pub translucent: bool,
}

#[derive(Component, Clone, Debug)]
pub struct ChampionView {
    pub id: EntityId,
    pub model: SpawnedModel,
    pub animator: Animator,
    scale: f32,
    carry: Option<f32>,
    drop_angle: Option<f32>,
    time: f32,
    dead: bool,
    moving: bool,
    server_anim: String,
    played: String,
    recalling: bool,
    stunned: bool,
    death_t: f32,
    last_pos: Option<Vec3>,
    speed: f32,
    lift_start: f32,
    lift_dur: f32,
    invisible: bool,
    final_pose: Pose,
    yaw: Option<f32>,
    yaw_rate: f32,
    lean: f32,
    flinch_amt: f32,
    carry_w: f32,
    max_hp: f32,
}

impl ChampionView {
    pub fn new(id: EntityId, def: &ChampionModelDef, model: SpawnedModel) -> Self {
        let mut clips = generic_clips();
        clips.extend(def.clips.iter().copied());
        let ctx = ClipCtx {
            hand: def.hand.unwrap_or(DEFAULT_CTX.hand),
            energy: def.energy.unwrap_or(DEFAULT_CTX.energy),
        };
        Self {
            id,
            model,
            animator: Animator::new(clips, ctx),
            scale: def.scale,
            carry: def.carry,
            drop_angle: def.drop_angle,
            time: 0.0,
            dead: false,
            moving: false,
            server_anim: "idle".into(),
            played: String::new(),
            recalling: false,
            stunned: false,
            death_t: -1.0,
            last_pos: None,
            speed: 0.0,
            lift_start: -1.0,
            lift_dur: 0.0,
            invisible: false,
            final_pose: [0.0; POSE_SIZE],
            yaw: None,
            yaw_rate: 0.0,
            lean: 0.0,
            flinch_amt: 0.0,
            carry_w: 1.0,
            max_hp: 1.0,
        }
    }

    // ------------------------------------------------------------ contrat de la vue

    /// Animation demandée par le serveur, appelée quand elle change.
    pub fn play_anim(&mut self, name: &str) {
        if name == "death" {
            return self.die();
        }
        if self.dead {
            self.respawn();
        }
        if name.starts_with("cast_") {
            self.animator.play(name, None, None);
            return;
        }
        self.server_anim = name.to_owned();
        if name == "run" || name == "dash" {
            self.recalling = false;
        }
        self.refresh_base();
    }

    /// Recul quand le champion encaisse un coup (additif, s'estompe en ~0,3 s).
    pub fn flinch(&mut self, amount: f32) {
        if self.dead {
            return;
        }
        let k = (0.35 + amount / self.max_hp * 6.0).min(1.0);
        self.flinch_amt = self.flinch_amt.max(k).min(1.0);
    }

    pub fn strike(&mut self) {
        if !self.dead {
            self.animator.play("strike", Some(0.04), None);
        }
    }

    pub fn cast(&mut self, slot: &str) {
        if !self.dead {
            self.animator
                .play(&format!("cast_{slot}"), Some(0.08), None);
        }
    }

    pub fn set_recalling(&mut self, on: bool) {
        self.recalling = on;
        self.refresh_base();
    }

    /// Hauteur du modèle (monde), pour placer des FX au-dessus de la tête.
    pub fn height(&self, spec_height: f32) -> f32 {
        spec_height * self.scale
    }

    /// Une frame : `pos` est la position affichée de la vue (monde), `facing` son orientation.
    pub fn step(
        &mut self,
        snap: &EntitySnapshot,
        anim: &str,
        dt: f32,
        pos: Vec3,
        facing: f32,
    ) -> Frame {
        if anim != self.played {
            self.played = anim.to_owned();
            self.play_anim(anim);
        }
        self.time += dt;
        let snap_dead = snap.dead == Some(true);
        if snap_dead && !self.dead {
            self.die();
        } else if !snap_dead && self.dead {
            self.respawn();
        }

        // vitesse affichée (la position est posée par le placement des vues)
        if let Some(last) = self.last_pos
            && dt > 0.0
        {
            let v = Vec2::new(pos.x - last.x, pos.z - last.z).length() / dt;
            self.speed += (v - self.speed) * (dt * 12.0).min(1.0);
        }
        self.last_pos = Some(pos);
        let moving = if self.moving {
            self.speed > 0.35
        } else {
            self.speed > 0.8
        };
        let mut stunned = false;
        let mut knockup = -1.0f32;
        let mut invisible = false;
        for b in snap.buffs.iter().flatten() {
            if STUN_BUFFS.contains(&b.id.as_str()) {
                stunned = true;
            }
            if b.id == "knockup" {
                knockup = knockup.max(b.remaining as f32);
            }
            if b.id == "invisible" {
                invisible = true;
            }
        }
        let lift = self.update_lift(knockup);
        self.invisible = invisible;
        if moving != self.moving || stunned != self.stunned {
            self.moving = moving;
            self.stunned = stunned;
            self.refresh_base();
        }
        self.animator.rate = if self.animator.base_clip() == "run" {
            (self.speed / NOMINAL_SPEED).clamp(0.6, 1.7)
        } else {
            1.0
        };

        self.max_hp = if snap.max_hp > 0.0 {
            snap.max_hp as f32
        } else {
            1.0
        };
        let model_yaw = self.update_yaw(dt, facing);
        let (model_visible, outlines_visible) = self.tick_death(dt);
        let pose = *self.animator.update(dt);
        self.layer(&pose, dt);
        Frame {
            joints: self.model.skeleton.transforms(&self.final_pose),
            model_yaw,
            model_lift: lift,
            model_visible,
            outlines_visible: outlines_visible && !self.invisible,
            translucent: self.invisible,
        }
    }

    // ------------------------------------------------------------ interne

    /// Orientation lissée : la vue suit l'orientation serveur ; le modèle reçoit l'écart pour que
    /// l'orientation affichée tourne progressivement (fini les demi-tours instantanés).
    fn update_yaw(&mut self, dt: f32, target: f32) -> f32 {
        let yaw = self.yaw.get_or_insert(target);
        let d = wrap_angle(target - *yaw);
        let step = d * (1.0 - (-dt * 14.0).exp());
        *yaw += step;
        let rate = if dt > 0.0 { step / dt } else { 0.0 };
        self.yaw_rate += (rate - self.yaw_rate) * (dt * 10.0).min(1.0);
        wrap_angle(*yaw - target)
    }

    /// Couches additives : inclinaison (vitesse, virage), recul, port d'arme.
    fn layer(&mut self, pose: &Pose, dt: f32) {
        let p = &mut self.final_pose;
        *p = *pose;
        let moving = self.animator.base_clip() == "run" && !self.dead;
        let want = if moving {
            (self.speed / NOMINAL_SPEED).min(1.3)
        } else {
            0.0
        };
        self.lean += (want - self.lean) * (dt * 8.0).min(1.0);
        // en avant selon la vitesse, et vers l'intérieur du virage
        p[BODY * 3 + 2] -= 0.1 * self.lean;
        p[BODY * 3] += (-self.yaw_rate * 0.045).clamp(-0.22, 0.22) * self.lean;
        // recul : buste et tête rejetés en arrière
        if self.flinch_amt > 0.001 {
            let f = self.flinch_amt;
            p[SPINE * 3 + 2] += 0.32 * f;
            p[HEAD * 3 + 2] += 0.22 * f;
            p[ARM_L * 3] += 0.15 * f;
            p[ARM_R * 3] -= 0.15 * f;
            p[BODY * 3 + 2] += 0.06 * f;
            self.flinch_amt *= (-dt * 11.0).exp();
        }
        // port d'arme : l'accessoire compense la chaîne corps → torse → bras → avant-bras ; à la mort, il se
        // couche
        let dying = self.animator.current() == "death" && self.drop_angle.is_some();
        let angle = if dying { self.drop_angle } else { self.carry };
        if let Some(angle) = angle {
            let want = if dying || CARRY_CLIPS.contains(&self.animator.current()) {
                1.0
            } else {
                0.0
            };
            self.carry_w += (want - self.carry_w) * (dt * 10.0).min(1.0);
            if self.carry_w > 0.001 {
                let chain =
                    p[BODY * 3 + 2] + p[SPINE * 3 + 2] + p[ARM_R * 3 + 2] + p[FORE_R * 3 + 2];
                let i = PROP * 3 + 2;
                p[i] += (angle - chain - p[i]) * self.carry_w;
            }
        }
    }

    /// Projection : le modèle décrit une parabole pendant la durée du knockup (la simulation reste au
    /// sol). Renvoie la hauteur du modèle.
    fn update_lift(&mut self, remaining: f32) -> f32 {
        if remaining > 0.0 && self.lift_start < 0.0 {
            self.lift_start = self.time;
            self.lift_dur = remaining.max(0.2);
        }
        if self.lift_start < 0.0 {
            return 0.0;
        }
        let k = (self.time - self.lift_start) / self.lift_dur;
        if k >= 1.0 || (remaining <= 0.0 && k > 0.95) {
            self.lift_start = -1.0;
            return 0.0;
        }
        4.0 * (0.35 + self.lift_dur * 0.6).min(1.1) * k * (1.0 - k)
    }

    fn refresh_base(&mut self) {
        if self.dead {
            return;
        }
        let base = if self.stunned {
            "stunned"
        } else if self.recalling {
            "recall"
        } else if self.server_anim == "dash" {
            "dash"
        } else if self.moving {
            "run"
        } else if self.server_anim == "attack" {
            "attackReady"
        } else if self.server_anim == "run" {
            "run"
        } else {
            "idle"
        };
        self.animator.set_base(base);
    }

    fn die(&mut self) {
        if self.dead {
            return;
        }
        self.dead = true;
        self.recalling = false;
        self.death_t = 0.0;
        self.animator.play("death", Some(0.08), None);
    }

    fn respawn(&mut self) {
        self.dead = false;
        self.death_t = -1.0;
        self.animator.set_base("idle");
        self.animator.play("idle", Some(0.0), None);
    }

    /// Chute (0,8 s) puis dissolution (1,2 s), puis invisible jusqu'à la réapparition. Renvoie la
    /// visibilité du modèle et des contours.
    fn tick_death(&mut self, dt: f32) -> (bool, bool) {
        if self.death_t < 0.0 {
            return (true, true);
        }
        self.death_t += dt;
        let k = (self.death_t - 0.8) / 1.2;
        (k < 1.0, k <= 0.0)
    }
}

/// Anime les champions et écrit squelette, modèle et matériaux.
pub fn animate_champions(
    time: Res<Time>,
    state: Res<crate::net::GameState>,
    mut views: Query<(&EntityView, &mut ChampionView, &Transform)>,
    mut transforms: Query<&mut Transform, Without<ChampionView>>,
    mut visibility: Query<&mut Visibility>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs().min(0.1);
    for (view, mut champ, at) in &mut views {
        let Some(snap) = state.entities().get(&champ.id) else {
            continue;
        };
        let was_translucent = champ.invisible;
        let (facing, _, _) = at.rotation.to_euler(EulerRot::YXZ);
        let f = champ.step(snap, &view.anim, dt, at.translation, facing);
        let m = &champ.model;
        for (j, t) in m.skeleton.joints.iter().zip(f.joints) {
            if let Ok(mut tr) = transforms.get_mut(*j) {
                *tr = t;
            }
        }
        if let Ok(mut tr) = transforms.get_mut(m.skeleton.root) {
            tr.rotation = Quat::from_rotation_y(f.model_yaw);
            tr.translation.y = f.model_lift;
        }
        let shown = |on: bool| {
            if on {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            }
        };
        if let Ok(mut v) = visibility.get_mut(m.skeleton.root) {
            v.set_if_neq(shown(f.model_visible));
        }
        for o in &m.outlines {
            if let Ok(mut v) = visibility.get_mut(*o) {
                v.set_if_neq(shown(f.outlines_visible));
            }
        }
        // invisibilité : modèle translucide (le matériau est propre à ce champion)
        if f.translucent != was_translucent {
            let mats = &m.materials;
            for h in [&mats.body, &mats.metal, &mats.glow] {
                if let Some(mut mat) = materials.get_mut(h) {
                    mat.base_color
                        .set_alpha(if f.translucent { 0.3 } else { 1.0 });
                    mat.alpha_mode = if f.translucent {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Opaque
                    };
                }
            }
        }
    }
}

/// Route les événements du serveur vers les vues (`route` de `index.ts`).
pub fn route_events(
    mut events: MessageReader<ServerEvent>,
    mut views: Query<&mut ChampionView>,
    mut melenchons: Query<&mut MelenchonView>,
) {
    for ServerEvent(ev) in events.read() {
        let target = match ev {
            GameEvent::AutoAttack { source, .. } | GameEvent::AbilityCast { source, .. } => *source,
            GameEvent::Damage { target, amount, .. } if *amount > 0.0 => *target,
            GameEvent::RecallStart { target } | GameEvent::RecallEnd { target, .. } => *target,
            _ => continue,
        };
        for mut m in &mut melenchons {
            if m.id == target {
                melenchon::route(&mut m, ev);
            }
        }
        for mut v in &mut views {
            if v.id != target {
                continue;
            }
            match ev {
                GameEvent::AutoAttack { .. } => v.strike(),
                GameEvent::AbilityCast { slot, .. } => v.cast(match slot {
                    AbilitySlot::Q => "Q",
                    AbilitySlot::W => "W",
                    AbilitySlot::E => "E",
                    AbilitySlot::R => "R",
                }),
                GameEvent::Damage { amount, .. } => v.flinch(*amount as f32),
                GameEvent::RecallStart { .. } => v.set_recalling(true),
                GameEvent::RecallEnd { .. } => v.set_recalling(false),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::generic::generic_for;
    use crate::render::champions::model::ChampionMaterials;
    use crate::render::champions::rig::Skeleton;
    use lov_shared::schema::ChampionClass;
    use lov_shared::types::{BuffSnapshot, EntityKind, TeamOrNeutral};

    fn view() -> ChampionView {
        let def = generic_for(ChampionClass::Fighter);
        let model = SpawnedModel {
            skeleton: Skeleton {
                root: Entity::PLACEHOLDER,
                joints: [Entity::PLACEHOLDER; JOINT_COUNT],
                rest: def.rig.rest_positions(),
            },
            materials: ChampionMaterials {
                body: Handle::default(),
                metal: Handle::default(),
                glow: Handle::default(),
            },
            meshes: vec![],
            outlines: vec![],
            glows: vec![],
        };
        ChampionView::new(1, &def, model)
    }

    fn snap() -> EntitySnapshot {
        EntitySnapshot {
            id: 1,
            kind: EntityKind::Champion,
            team: TeamOrNeutral::Blue,
            x: 0.0,
            y: 0.0,
            facing: 0.0,
            hp: 500.0,
            max_hp: 1000.0,
            mana: None,
            max_mana: None,
            shield: None,
            dead: None,
            subtype: Some("kaelen".into()),
            owner_id: None,
            level: None,
            anim: None,
            buffs: None,
            champ: None,
        }
    }

    #[test]
    fn la_vitesse_mesuree_lance_la_course() {
        let mut v = view();
        let s = snap();
        for i in 0..30 {
            v.step(
                &s,
                "idle",
                1.0 / 30.0,
                Vec3::new(i as f32 * 0.12, 0.0, 0.0),
                0.0,
            );
        }
        assert_eq!(v.animator.base_clip(), "run");
        assert!(
            v.animator.rate > 1.0,
            "3,6 monde/s : cadence au-dessus du nominal"
        );
    }

    #[test]
    fn frappe_et_incantation_par_evenement() {
        let mut v = view();
        let s = snap();
        v.step(&s, "idle", 0.016, Vec3::ZERO, 0.0);
        v.strike();
        assert_eq!(v.animator.current(), "strike");
        v.cast("R");
        assert_eq!(v.animator.current(), "cast_R");
        v.cast("Q");
        assert_eq!(v.animator.current(), "cast");
    }

    #[test]
    fn la_mort_couche_puis_cache_le_modele() {
        let mut v = view();
        let mut s = snap();
        s.dead = Some(true);
        let f = v.step(&s, "death", 0.5, Vec3::ZERO, 0.0);
        assert!(f.model_visible && f.outlines_visible);
        let f = v.step(&s, "death", 0.5, Vec3::ZERO, 0.0);
        assert!(
            f.model_visible && !f.outlines_visible,
            "dissolution : contour retiré"
        );
        let f = v.step(&s, "death", 1.2, Vec3::ZERO, 0.0);
        assert!(!f.model_visible);
        assert_eq!(v.animator.current(), "death");
        // réapparition
        s.dead = None;
        let f = v.step(&s, "idle", 0.016, Vec3::ZERO, 0.0);
        assert!(f.model_visible && f.outlines_visible);
        assert_eq!(v.animator.current(), "idle");
    }

    #[test]
    fn l_orientation_tourne_progressivement() {
        let mut v = view();
        let s = snap();
        v.step(&s, "idle", 0.016, Vec3::ZERO, 0.0);
        // demi-tour demandé : le modèle garde l'ancienne orientation, puis la rattrape
        let f = v.step(&s, "idle", 0.016, Vec3::ZERO, 3.0);
        assert!(f.model_yaw.abs() > 2.0);
        let mut last = f.model_yaw;
        for _ in 0..60 {
            last = v.step(&s, "idle", 0.016, Vec3::ZERO, 3.0).model_yaw;
        }
        assert!(last.abs() < 0.01);
    }

    #[test]
    fn projection_en_parabole() {
        let mut v = view();
        let mut s = snap();
        s.buffs = Some(vec![BuffSnapshot {
            id: "knockup".into(),
            remaining: 1.0,
            stacks: None,
        }]);
        v.step(&s, "idle", 0.0, Vec3::ZERO, 0.0);
        let mid = v.step(&s, "idle", 0.5, Vec3::ZERO, 0.0).model_lift;
        assert!((mid - 4.0 * 0.95 * 0.25).abs() < 1e-5, "{mid}");
        assert_eq!(v.animator.base_clip(), "stunned");
        s.buffs = None;
        assert_eq!(v.step(&s, "idle", 0.6, Vec3::ZERO, 0.0).model_lift, 0.0);
    }

    #[test]
    fn invisible_et_recul() {
        let mut v = view();
        let mut s = snap();
        s.buffs = Some(vec![BuffSnapshot {
            id: "invisible".into(),
            remaining: 3.0,
            stacks: None,
        }]);
        let f = v.step(&s, "idle", 0.016, Vec3::ZERO, 0.0);
        assert!(f.translucent && !f.outlines_visible);
        let before = v.step(&s, "idle", 0.016, Vec3::ZERO, 0.0).joints[SPINE].rotation;
        v.flinch(100.0);
        let after = v.step(&s, "idle", 0.016, Vec3::ZERO, 0.0).joints[SPINE].rotation;
        assert!(before.angle_between(after) > 0.2, "buste rejeté en arrière");
    }

    #[test]
    fn angles_ramenes() {
        assert!((wrap_angle(3.5 * TAU / 2.0) + TAU / 4.0).abs() < 1e-5);
        assert!((wrap_angle(-0.5) + 0.5).abs() < 1e-6);
    }
}
