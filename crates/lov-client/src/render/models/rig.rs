//! Rig procédural générique et animateur des modèles low-poly.
//!
//! Portage de `RigView` (`packages/client/src/render/models/rig.ts`). Un modèle se compose de pivots
//! nommés (jambes, bras, ailes, queue…) : les géométries sont décalées pour que chaque pivot soit à
//! l'articulation, et l'animateur fait tourner les pivots selon l'animation courante
//! (idle / run / attack / cast_* / death). Le modèle regarde +x local.
//!
//! Les articulations gardent leurs angles d'Euler : l'animation ne change qu'un angle (`rotation.z` d'une
//! jambe) et laisse les autres à leur valeur de construction (`rotation.x` d'une patte de crabe), comme
//! three.js. Les transformations ne sont écrites qu'après le calcul de toute la pose.

use std::f32::consts::PI;

use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;

use super::kit::{Model, N, euler};

/// Description du rig, avec des indices de nœuds (`Rig<N>`) ou d'articulations animées (`Rig<usize>`).
#[derive(Clone, Debug)]
pub struct Rig<T> {
    /// Racine du modèle animé (enfant de la vue).
    pub root: T,
    /// Corps : rebond / inclinaison.
    pub body: Option<T>,
    pub head: Option<T>,
    /// Jambes : balancées en opposition (rotation autour de z local = avant/arrière quand on regarde +x).
    pub legs: Vec<T>,
    pub arms: Vec<T>,
    /// Bras/arme qui frappe pendant l'attaque (sinon `arms[0]`).
    pub weapon: Option<T>,
    pub wings: Vec<T>,
    pub tail: Option<T>,
    /// Éléments qui tournent en continu (cristaux).
    pub spinners: Vec<T>,
    /// Éléments qui flottent (rebond sinusoïdal).
    pub floaters: Vec<T>,
    /// Hauteur totale (pour la mort/enfoncement).
    pub height: f32,
    /// Fréquence de pas (cycles / s) en course.
    pub stride: f32,
    /// Quadrupède : jambes avant/arrière alternées en diagonale.
    pub quadruped: bool,
    /// Pas d'animation de chute à la mort (structures : effondrement).
    pub structure: bool,
}

impl Rig<N> {
    /// Rig vide d'un modèle : seule la racine, hauteur et foulée par défaut.
    pub fn new(height: f32) -> Self {
        Self {
            root: Model::ROOT,
            body: None,
            head: None,
            legs: Vec::new(),
            arms: Vec::new(),
            weapon: None,
            wings: Vec::new(),
            tail: None,
            spinners: Vec::new(),
            floaters: Vec::new(),
            height,
            stride: 2.2,
            quadruped: false,
            structure: false,
        }
    }
}

impl<T: Copy> Rig<T> {
    fn map<U>(&self, mut f: impl FnMut(T) -> U) -> Rig<U> {
        let mut all = |v: &[T]| v.iter().map(|&x| f(x)).collect::<Vec<_>>();
        let legs = all(&self.legs);
        let arms = all(&self.arms);
        let wings = all(&self.wings);
        let spinners = all(&self.spinners);
        let floaters = all(&self.floaters);
        Rig {
            root: f(self.root),
            body: self.body.map(&mut f),
            head: self.head.map(&mut f),
            weapon: self.weapon.map(&mut f),
            tail: self.tail.map(&mut f),
            legs,
            arms,
            wings,
            spinners,
            floaters,
            height: self.height,
            stride: self.stride,
            quadruped: self.quadruped,
            structure: self.structure,
        }
    }
}

/// Articulation animée : sa pose courante et sa position de construction.
#[derive(Clone, Debug)]
pub struct Joint {
    pub entity: Entity,
    pub pos: Vec3,
    pub rot: Vec3,
    pub scale: Vec3,
    pub base: Vec3,
}

/// Animateur d'un modèle low-poly, sur l'entité de la vue.
#[derive(Component, Clone, Debug)]
pub struct RigView {
    pub joints: Vec<Joint>,
    pub rig: Rig<usize>,
    anim: String,
    t: f32,
    anim_t: f32,
    death_t: f32,
    move_blend: f32,
}

impl RigView {
    /// Animateur du rig `rig` d'un modèle apparu (`entities` : une entité par nœud du modèle).
    /// `phase` décale l'horloge (le client web la tire au hasard entre 0 et 10 s).
    pub fn new(model: &Model, rig: &Rig<N>, entities: &[Entity], phase: f32) -> Self {
        let mut joints: Vec<Joint> = Vec::new();
        let mut index: Vec<Option<usize>> = vec![None; model.nodes.len()];
        let rig = rig.map(|n| {
            *index[n].get_or_insert_with(|| {
                let node = &model.nodes[n];
                joints.push(Joint {
                    entity: entities[n],
                    pos: node.pos,
                    rot: node.rot,
                    scale: node.scale,
                    base: node.pos,
                });
                joints.len() - 1
            })
        });
        Self {
            joints,
            rig,
            anim: "idle".into(),
            t: phase,
            anim_t: 0.0,
            death_t: 0.0,
            move_blend: 0.0,
        }
    }

    pub fn play_anim(&mut self, name: &str) {
        if name != self.anim {
            self.anim_t = 0.0;
            self.anim = name.to_owned();
        }
    }

    /// Calcule la pose de la frame (`update` du client web).
    pub fn step(&mut self, dt: f32, dead: bool) {
        let r = &self.rig;
        let j = &mut self.joints;
        self.t += dt;
        self.anim_t += dt;

        for &s in &r.spinners {
            j[s].rot.y += dt * if dead { 0.0 } else { 0.9 };
        }
        for (i, &f) in r.floaters.iter().enumerate() {
            j[f].pos.y = j[f].base.y
                + if dead {
                    -0.3
                } else {
                    (self.t * 1.6 + i as f32).sin() * 0.12
                };
        }

        if dead {
            self.death_t = (self.death_t + dt * if r.structure { 0.6 } else { 1.4 }).min(1.0);
            let k = self.death_t * self.death_t;
            let root = &mut j[r.root];
            if r.structure {
                root.pos.y = -r.height * 0.55 * k;
                root.rot.z = 0.12 * k;
                root.rot.x = 0.08 * k;
            } else {
                root.rot.x = PI / 2.0 * (self.death_t * 1.6).min(1.0);
                root.pos.y = -r.height * 0.6 * (self.death_t - 0.5).max(0.0) * 2.0;
            }
            return;
        }
        if self.death_t > 0.0 {
            self.death_t = 0.0;
            j[r.root].rot = Vec3::ZERO;
            j[r.root].pos.y = 0.0;
        }

        let running = self.anim == "run";
        self.move_blend +=
            (if running { 1.0 } else { 0.0 } - self.move_blend) * (dt * 10.0).min(1.0);
        let mb = self.move_blend;
        let ph = self.t * r.stride * PI * 2.0;

        // jambes
        for (i, &leg) in r.legs.iter().enumerate() {
            let phase = if r.quadruped {
                (if i % 2 == 0 { 0.0 } else { PI }) + if i >= 2 { PI } else { 0.0 }
            } else if i % 2 == 0 {
                0.0
            } else {
                PI
            };
            j[leg].rot.z = (ph + phase).sin() * 0.6 * mb;
        }
        // corps : respiration / rebond de course
        if let Some(b) = r.body {
            let breathe = (self.t * 2.0).sin() * 0.02;
            j[b].pos.y = j[b].base.y + breathe * (1.0 - mb) + ph.sin().abs() * 0.06 * mb;
            j[b].rot.z = -0.08 * mb;
        }
        // bras : balancier en course
        for (i, &arm) in r.arms.iter().enumerate() {
            j[arm].rot.z = (ph + if i % 2 == 1 { 0.0 } else { PI }).sin() * 0.5 * mb;
        }
        if !r.wings.is_empty() {
            let flap =
                (self.t * if running { 7.0 } else { 2.2 }).sin() * if running { 0.5 } else { 0.18 };
            for (i, &w) in r.wings.iter().enumerate() {
                j[w].rot.x = if i % 2 == 1 { -1.0 } else { 1.0 } * (0.25 + flap);
            }
        }
        if let Some(tail) = r.tail {
            j[tail].rot.y = (self.t * 3.0).sin() * 0.3;
        }
        if let Some(h) = r.head {
            j[h].rot.z = (self.t * 1.3).sin() * 0.05;
            j[h].pos.x = j[h].base.x;
        }

        // attaque / incantation : frappe avec l'arme
        if self.anim == "attack" || self.anim.starts_with("cast") {
            let k = (self.anim_t / 0.35).min(1.0);
            let swing = (k * PI).sin();
            if let Some(w) = r.weapon.or(r.arms.first().copied()) {
                j[w].rot.z = -1.6 * swing;
            }
            if let Some(b) = r.body {
                j[b].rot.z = 0.15 * swing;
            }
            if let Some(h) = r.head.filter(|_| r.weapon.is_none() && r.arms.is_empty()) {
                j[h].pos.x = j[h].base.x + 0.15 * swing; // morsure
            }
            if self.anim_t > 0.6 {
                self.anim_t = 0.0; // boucle si l'animation reste « attack »
            }
        }
    }

    /// Écrit la pose dans les transformations des articulations.
    pub fn flush<F: QueryFilter>(&self, transforms: &mut Query<&mut Transform, F>) {
        for joint in &self.joints {
            if let Ok(mut t) = transforms.get_mut(joint.entity) {
                *t = Transform {
                    translation: joint.pos,
                    rotation: euler(joint.rot),
                    scale: joint.scale,
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(structure: bool) -> RigView {
        let mut m = Model::new();
        let body = m.pivot(Model::ROOT, 0.0, 0.5, 0.0);
        let legs = vec![
            m.pivot(Model::ROOT, 0.0, 0.5, 0.1),
            m.pivot(Model::ROOT, 0.0, 0.5, -0.1),
        ];
        let crystal = m.pivot(Model::ROOT, 0.0, 3.0, 0.0);
        let mut rig = Rig::new(2.0);
        rig.body = Some(body);
        rig.legs = legs;
        rig.floaters = vec![crystal];
        rig.spinners = vec![crystal];
        rig.structure = structure;
        let entities: Vec<Entity> = (0..m.nodes.len() as u32)
            .map(Entity::from_raw_u32)
            .map(Option::unwrap)
            .collect();
        RigView::new(&m, &rig, &entities, 0.0)
    }

    #[test]
    fn une_articulation_partagee_n_est_animee_qu_une_fois() {
        let v = view(false);
        // racine, corps, 2 jambes, cristal (à la fois flotteur et toupie)
        assert_eq!(v.joints.len(), 5);
        assert_eq!(v.rig.floaters, v.rig.spinners);
    }

    #[test]
    fn les_jambes_ne_bougent_qu_en_course() {
        let mut v = view(false);
        for _ in 0..30 {
            v.step(1.0 / 30.0, false);
        }
        assert!(v.joints[v.rig.legs[0]].rot.z.abs() < 1e-6);
        v.play_anim("run");
        for _ in 0..7 {
            v.step(1.0 / 30.0, false);
        }
        let (a, b) = (v.joints[v.rig.legs[0]].rot.z, v.joints[v.rig.legs[1]].rot.z);
        assert!(a.abs() > 0.05, "jambe immobile en course : {a}");
        assert!((a + b).abs() < 1e-5, "jambes en opposition : {a} / {b}");
    }

    #[test]
    fn mort_puis_reapparition() {
        let mut v = view(false);
        for _ in 0..60 {
            v.step(1.0 / 30.0, true);
        }
        let root = &v.joints[v.rig.root];
        assert!((root.rot.x - PI / 2.0).abs() < 1e-5, "couché sur le flanc");
        assert!(root.pos.y < -1.0, "enfoncé dans le sol");
        v.step(1.0 / 30.0, false);
        let root = &v.joints[v.rig.root];
        assert_eq!((root.rot, root.pos.y), (Vec3::ZERO, 0.0));
    }

    #[test]
    fn les_structures_s_effondrent_sans_basculer() {
        let mut v = view(true);
        for _ in 0..120 {
            v.step(1.0 / 30.0, true);
        }
        let root = &v.joints[v.rig.root];
        assert!((root.pos.y + 2.0 * 0.55).abs() < 1e-5);
        assert!((root.rot.z - 0.12).abs() < 1e-5);
        // le cristal retombe et cesse de tourner
        let c = &v.joints[v.rig.floaters[0]];
        assert!((c.pos.y - 2.7).abs() < 1e-5);
    }

    #[test]
    fn l_attaque_balance_l_arme() {
        let mut m = Model::new();
        let arm = m.pivot(Model::ROOT, 0.0, 1.0, 0.0);
        let mut rig = Rig::new(1.0);
        rig.arms = vec![arm];
        let entities: Vec<Entity> = (0..2u32).filter_map(Entity::from_raw_u32).collect();
        let mut v = RigView::new(&m, &rig, &entities, 0.0);
        v.play_anim("attack");
        v.step(0.175, false);
        assert!(
            (v.joints[v.rig.arms[0]].rot.z + 1.6).abs() < 1e-4,
            "frappe à mi-course"
        );
    }
}
