//! Poses et animateur procédural des champions (logique pure, sans rendu, testée).
//!
//! Portage de `packages/client/src/render/champions/anim/pose.ts` et `ease.ts`.
//!
//! Conventions (le modèle regarde vers +x local) : avant = +x, haut = +y, droite = +z.
//! Un membre pendant tourné de +θ autour de z part VERS L'AVANT ; le torse tourné de +θ autour de z part EN
//! ARRIÈRE. Écarter un membre vers l'extérieur : rotation x de −θ côté droit (+z), +θ côté gauche.

use std::collections::HashMap;

/// Articulations, dans l'ordre du squelette (`J` du client web).
pub mod j {
    /// Racine du corps (inclinaison globale).
    pub const BODY: usize = 0;
    pub const HIPS: usize = 1;
    pub const SPINE: usize = 2;
    pub const HEAD: usize = 3;
    pub const ARM_L: usize = 4;
    pub const FORE_L: usize = 5;
    pub const ARM_R: usize = 6;
    pub const FORE_R: usize = 7;
    pub const LEG_L: usize = 8;
    pub const SHIN_L: usize = 9;
    pub const LEG_R: usize = 10;
    pub const SHIN_R: usize = 11;
    /// Arme / accessoire tenu.
    pub const PROP: usize = 12;
}

pub const JOINT_COUNT: usize = 13;
/// Indice du décalage de position du corps (x, y, z) dans une pose.
pub const BODY_OFFSET: usize = JOINT_COUNT * 3;
pub const POSE_SIZE: usize = BODY_OFFSET + 3;

/// Rotations (x, y, z) de chaque articulation, puis décalage du corps.
pub type Pose = [f32; POSE_SIZE];

pub fn set_rot(p: &mut Pose, joint: usize, x: f32, y: f32, z: f32) {
    p[joint * 3] = x;
    p[joint * 3 + 1] = y;
    p[joint * 3 + 2] = z;
}

pub fn add_rot(p: &mut Pose, joint: usize, x: f32, y: f32, z: f32) {
    p[joint * 3] += x;
    p[joint * 3 + 1] += y;
    p[joint * 3 + 2] += z;
}

pub fn set_offset(p: &mut Pose, x: f32, y: f32, z: f32) {
    p[BODY_OFFSET] = x;
    p[BODY_OFFSET + 1] = y;
    p[BODY_OFFSET + 2] = z;
}

/// `out = a + (b − a)·k`
pub fn blend_pose(a: &Pose, b: &Pose, k: f32, out: &mut Pose) {
    for i in 0..POSE_SIZE {
        out[i] = a[i] + (b[i] - a[i]) * k;
    }
}

// ---------------------------------------------------------------- easings (`ease.ts`)

/// Borne à 0..1.
pub fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - clamp01(t)).powi(3)
}

pub fn ease_in(t: f32) -> f32 {
    clamp01(t).powi(2)
}

/// Cloche 0 → 1 → 0 sur [a, b].
pub fn bell(t: f32, a: f32, b: f32) -> f32 {
    (clamp01((t - a) / (b - a)) * std::f32::consts::PI).sin()
}

// ---------------------------------------------------------------- clips

/// Contexte passé aux clips (paramètres propres au champion).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipCtx {
    /// Bras qui frappe : 1 = droit, −1 = gauche.
    pub hand: f32,
    /// Amplitude générale (tanks lourds < 1, assassins > 1).
    pub energy: f32,
}

/// Contexte par défaut.
pub const DEFAULT_CTX: ClipCtx = ClipCtx {
    hand: 1.0,
    energy: 1.0,
};

/// Écrit la pose à l'instant `t` (s depuis le début du clip). La pose arrive remise à zéro.
pub type Sample = fn(t: f32, out: &mut Pose, ctx: &ClipCtx);

#[derive(Clone, Copy, Debug)]
pub struct Clip {
    /// Boucle (idle, run…) ou one-shot (strike, cast…).
    pub looped: bool,
    /// Durée (s) pour un one-shot ; période indicative pour une boucle.
    pub duration: f32,
    pub sample: Sample,
}

pub type ClipSet = HashMap<&'static str, Clip>;

/// Chaîne de repli : `cast_Q` → `cast` → `idle` ; `strike_crit` → `strike` → `idle`…
pub fn resolve_clip<'a>(clips: &ClipSet, name: &'a str) -> &'a str {
    if clips.contains_key(name) {
        return name;
    }
    if let Some(i) = name.find('_')
        && i > 0
        && clips.contains_key(&name[..i])
    {
        return &name[..i];
    }
    "idle"
}

/// Nom de clip stable (clé de `ClipSet`) pour un nom résolu.
fn key(clips: &ClipSet, name: &str) -> &'static str {
    clips.get_key_value(name).map_or("idle", |(k, _)| *k)
}

/// Animateur : un clip courant + fondu depuis la pose précédente. Les one-shots reviennent
/// automatiquement au clip de base (idle/run/attackReady…) choisi par `set_base`.
#[derive(Clone, Debug)]
pub struct Animator {
    pub pose: Pose,
    /// Multiplicateur de vitesse des boucles (cadence de course selon la vitesse réelle).
    pub rate: f32,
    clips: ClipSet,
    ctx: ClipCtx,
    from: Pose,
    target: Pose,
    name: &'static str,
    base: &'static str,
    time: f32,
    blend: f32,
    blend_dur: f32,
    one_shot_speed: f32,
}

impl Animator {
    /// `clips` doit contenir `idle`.
    pub fn new(clips: ClipSet, ctx: ClipCtx) -> Self {
        Self {
            pose: [0.0; POSE_SIZE],
            rate: 1.0,
            clips,
            ctx,
            from: [0.0; POSE_SIZE],
            target: [0.0; POSE_SIZE],
            name: "idle",
            base: "idle",
            time: 0.0,
            blend: 1.0,
            blend_dur: 0.15,
            one_shot_speed: 1.0,
        }
    }

    pub fn current(&self) -> &'static str {
        self.name
    }

    pub fn base_clip(&self) -> &'static str {
        self.base
    }

    fn clip(&self, name: &str) -> Clip {
        self.clips
            .get(name)
            .or_else(|| self.clips.get("idle"))
            .copied()
            .unwrap_or(Clip {
                looped: true,
                duration: 1.0,
                sample: |_, _, _| {},
            })
    }

    /// Clip de fond (boucle) : appliqué tout de suite si aucun one-shot n'est en cours.
    pub fn set_base(&mut self, name: &str) {
        let n = key(&self.clips, resolve_clip(&self.clips, name));
        if n == self.base {
            return;
        }
        self.base = n;
        if self.clips.get(self.name).is_none_or(|c| c.looped) {
            self.start(n, 0.18, 1.0);
        }
    }

    /// Joue un clip ; un one-shot redémarre toujours, une boucle déjà active continue.
    pub fn play(&mut self, name: &str, fade: Option<f32>, speed: Option<f32>) {
        let n = key(&self.clips, resolve_clip(&self.clips, name));
        if self.clip(n).looped && n == self.name {
            return;
        }
        self.start(n, fade.unwrap_or(0.1), speed.unwrap_or(1.0));
    }

    fn start(&mut self, name: &'static str, fade: f32, speed: f32) {
        self.from = self.pose;
        self.name = name;
        self.time = 0.0;
        self.blend = if fade > 0.0 { 0.0 } else { 1.0 };
        self.blend_dur = fade;
        self.one_shot_speed = speed;
    }

    pub fn update(&mut self, dt: f32) -> &Pose {
        let clip = self.clip(self.name);
        self.time += dt
            * if clip.looped {
                self.rate
            } else {
                self.one_shot_speed
            };
        if !clip.looped && self.time >= clip.duration && self.name != self.base {
            // fin du one-shot : on ne revient pas au fond si c'est un état final (mort)
            if self.name != "death" {
                self.start(self.base, 0.2, 1.0);
            }
        }
        let c = self.clip(self.name);
        self.target = [0.0; POSE_SIZE];
        let t = if c.looped {
            self.time
        } else {
            self.time.min(c.duration)
        };
        (c.sample)(t, &mut self.target, &self.ctx);
        if self.blend < 1.0 {
            self.blend = (self.blend + dt / self.blend_dur.max(1e-3)).min(1.0);
            let k = self.blend * self.blend * (3.0 - 2.0 * self.blend); // smoothstep
            let (from, target) = (self.from, self.target);
            blend_pose(&from, &target, k, &mut self.pose);
        } else {
            self.pose = self.target;
        }
        &self.pose
    }
}
