//! Primitives d'effets composables : télégraphie, onde de choc, éclat lumineux, zone persistante, suivi
//! d'entité, projectile, traînée de dash, éclair. Les FX des sorts sont des assemblages de ces briques.
//!
//! Portage de `packages/client/src/fx/effects/primitives.ts`. Toutes les positions sont en coordonnées
//! monde (`Fx::world_pos` depuis les événements). Les primitives copient ce qu'elles conservent.
//!
//! Écart avec le web : un effet n'est pas un objet three.js qu'on modifie en direct, mais des entités Bevy
//! (décal, sprite, ruban) dont on édite le matériau et la transformation : les petites fonctions
//! [`edit_decal`], [`set_sprite`], [`push_ribbon`] et [`ribbon_color`] encapsulent ces accès.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use lov_shared::types::EntityId;

use crate::fx::core::curves::ease;
use crate::fx::core::rng::{Range, Rng};
use crate::fx::core::runtime::{Effect, Fx, FxDecal, FxRibbon, FxSprite};
use crate::fx::particles::system::{Blend, ParticleStyle};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::ProjectileHandle;
use crate::fx::shaders::{
    DecalMaterial, DecalShape, GroundDecal, Ribbon, RibbonMaterial, RibbonMode,
};

thread_local! {
    /// Aléatoire des FX (le `rng` global du client web) : les FX tournent dans un système exclusif.
    static RNG: RefCell<Rng> = RefCell::new(Rng::default());
}

/// Nombre aléatoire dans `0..1`.
pub fn rand() -> f32 {
    RNG.with(|r| r.borrow_mut().next_f32())
}

/// Nombre aléatoire dans `min..max`.
pub fn rand_range(min: f32, max: f32) -> f32 {
    RNG.with(|r| r.borrow_mut().range(min, max))
}

/// -1 ou 1.
pub fn rand_sign() -> f32 {
    RNG.with(|r| r.borrow_mut().sign())
}

/// Plage `[min, max]` (raccourci des options d'émission).
pub const fn between(min: f32, max: f32) -> Option<Range> {
    Some(Range::Between(min, max))
}

// ---------------------------------------------------------------------------------- accès aux entités

/// Modifie le matériau d'un décal ; si `f` renvoie un placement (`decal.circle`/`rect`/`cone`), il est posé.
pub fn edit_decal(
    fx: &mut Fx,
    d: &FxDecal,
    f: impl FnOnce(&mut DecalMaterial, &GroundDecal) -> Option<Transform>,
) {
    let world = fx.world();
    let tf = world
        .resource_mut::<Assets<DecalMaterial>>()
        .get_mut(&d.material)
        .and_then(|mut m| f(&mut m, &d.decal));
    if let (Some(tf), Some(mut cur)) = (tf, world.get_mut::<Transform>(d.entity)) {
        *cur = tf;
    }
}

/// Place un décal en cercle (ou anneau) sans toucher au reste du matériau.
pub fn decal_circle(fx: &mut Fx, d: &FxDecal, center: Vec3, radius: f32, shape: DecalShape) {
    edit_decal(fx, d, |m, g| Some(g.circle(m, center, radius, shape)));
}

/// Opacité d'un décal.
pub fn decal_opacity(fx: &mut Fx, d: &FxDecal, opacity: f32) {
    edit_decal(fx, d, |m, _| {
        m.u.opacity = opacity;
        None
    });
}

/// Position, taille (monde) et couleur d'un sprite ; `opacity` éteint l'éclat (mélange additif).
pub fn set_sprite(fx: &mut Fx, sp: &FxSprite, pos: Vec3, size: f32, color: Vec3, opacity: f32) {
    let world = fx.world();
    if let Some(mut m) = world
        .resource_mut::<Assets<StandardMaterial>>()
        .get_mut(&sp.material)
    {
        let c = color * opacity.clamp(0.0, 1.0);
        m.base_color = Color::LinearRgba(LinearRgba::rgb(c.x, c.y, c.z));
    }
    if let Some(mut tf) = world.get_mut::<Transform>(sp.entity) {
        tf.translation = pos;
        tf.scale = Vec3::new(size, size, 1.0);
    }
}

/// Montre ou cache un sprite (sans le rendre au pool).
pub fn sprite_visible(fx: &mut Fx, sp: &FxSprite, on: bool) {
    if let Some(mut v) = fx.world().get_mut::<Visibility>(sp.entity) {
        v.set_if_neq(if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// Ajoute un point à un ruban.
pub fn push_ribbon(fx: &mut Fx, r: &FxRibbon, p: Vec3) {
    if let Some(mut ribbon) = fx.world().get_mut::<Ribbon>(r.entity) {
        ribbon.push(p);
    }
}

/// Points vivants d'un ruban (0 = effacé).
pub fn ribbon_alive(fx: &mut Fx, r: &FxRibbon) -> usize {
    fx.world().get::<Ribbon>(r.entity).map_or(0, Ribbon::alive)
}

/// Réglages d'un ruban (largeur, durée de vie, mode, distance minimale).
pub fn ribbon_setup(fx: &mut Fx, r: &FxRibbon, f: impl FnOnce(&mut Ribbon)) {
    if let Some(mut ribbon) = fx.world().get_mut::<Ribbon>(r.entity) {
        f(&mut ribbon);
    }
}

/// Couleurs d'un ruban (`setColor(color, core)`).
pub fn ribbon_color(fx: &mut Fx, r: &FxRibbon, color: Vec3, core: Option<Vec3>) {
    if let Some(mut m) = fx
        .world()
        .resource_mut::<Assets<RibbonMaterial>>()
        .get_mut(&r.material)
    {
        *m = RibbonMaterial::new(color, core);
    }
}

// ---------------------------------------------------------------------------------- effets génériques

type StepFn = Box<dyn FnMut(&mut Fx, f32, f32) + Send + Sync>;
type DoneFn = Box<dyn FnOnce(&mut Fx) + Send + Sync>;

/// Effet piloté par une fonction de progression `t ∈ 0..1` (`timed`).
struct Timed {
    elapsed: f32,
    duration: f32,
    step: StepFn,
    done: Option<DoneFn>,
}

impl Effect for Timed {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.elapsed += dt;
        let t = (self.elapsed / self.duration.max(1e-3)).min(1.0);
        (self.step)(fx, t, dt);
        t < 1.0
    }

    fn end(&mut self, fx: &mut Fx) {
        if let Some(done) = self.done.take() {
            done(fx);
        }
    }
}

/// Effet générique piloté par `step(fx, t, dt)` pendant `duration`, puis `done`.
pub fn timed(
    fx: &mut Fx,
    duration: f32,
    step: impl FnMut(&mut Fx, f32, f32) + Send + Sync + 'static,
    done: impl FnOnce(&mut Fx) + Send + Sync + 'static,
) {
    fx.add(Timed {
        elapsed: 0.0,
        duration,
        step: Box::new(step),
        done: Some(Box::new(done)),
    });
}

/// Sans rien à faire à la fin.
pub fn nothing(_: &mut Fx) {}

/// Forme d'une télégraphie.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TeleShape {
    #[default]
    Circle,
    /// Rectangle jusqu'au point visé, de largeur totale `width` (défaut : le rayon).
    Rect { to: Vec3, width: Option<f32> },
    /// Cône vers le point visé, de demi-angle `half_angle` (rad).
    Cone { to: Vec3, half_angle: f32 },
}

/// Options d'une télégraphie (`TelegraphOptions`).
#[derive(Clone, Copy, Debug, Default)]
pub struct TelegraphOptions {
    pub shape: TeleShape,
    /// Remplissage de fond (défaut 0,12).
    pub fill: Option<f32>,
    /// Couleur du remplissage progressif (défaut : la couleur).
    pub color2: Option<Vec3>,
}

/// Télégraphie au sol qui se remplit pendant `duration`, puis disparaît rapidement. `on_done` part à la fin
/// du remplissage (explosion…).
pub fn telegraph_then(
    fx: &mut Fx,
    center: Vec3,
    radius: f32,
    duration: f32,
    color: Vec3,
    o: TelegraphOptions,
    on_done: impl FnOnce(&mut Fx) + Send + Sync + 'static,
) {
    let d = fx.acquire_decal();
    let (c1, c2) = (
        fx.ground_color(color),
        fx.ground_color(o.color2.unwrap_or(color)),
    );
    edit_decal(fx, &d, |m, g| {
        m.set_color(c1, c2);
        m.u.fill = o.fill.unwrap_or(0.12);
        m.u.progress = 0.0;
        m.u.edge = 0.06;
        m.u.opacity = 0.0;
        Some(match o.shape {
            TeleShape::Rect { to, width } => g.rect(m, center, to, width.unwrap_or(radius)),
            TeleShape::Cone { to, half_angle } => g.cone(m, center, to, radius, half_angle),
            TeleShape::Circle => g.circle(m, center, radius, DecalShape::Circle),
        })
    });
    let d2 = d.clone();
    timed(
        fx,
        duration,
        move |fx, t, _| {
            edit_decal(fx, &d, |m, _| {
                m.u.progress = t;
                m.u.opacity = (t * 6.0).min(1.0);
                None
            });
        },
        move |fx| {
            on_done(fx);
            edit_decal(fx, &d2, |m, _| {
                m.u.progress = -1.0;
                None
            });
            let d3 = d2.clone();
            timed(
                fx,
                0.15,
                move |fx, t, _| decal_opacity(fx, &d2, 1.0 - t),
                move |fx| fx.release_decal(d3),
            );
        },
    );
}

/// Télégraphie sans suite.
pub fn telegraph(
    fx: &mut Fx,
    center: Vec3,
    radius: f32,
    duration: f32,
    color: Vec3,
    o: TelegraphOptions,
) {
    telegraph_then(fx, center, radius, duration, color, o, nothing);
}

/// Anneau au sol qui s'étend de 0 à `radius` en s'estompant.
pub fn shockwave(fx: &mut Fx, at: Vec3, radius: f32, duration: f32, color: Vec3, edge: f32) {
    let d = fx.acquire_decal();
    edit_decal(fx, &d, |m, _| {
        m.set_color(color, color);
        m.u.edge = edge;
        None
    });
    let d2 = d.clone();
    timed(
        fx,
        duration,
        move |fx, t, _| {
            edit_decal(fx, &d, |m, g| {
                m.u.opacity = 1.0 - t * t;
                Some(g.circle(
                    m,
                    at,
                    (radius * ease::out_cubic(t)).max(0.01),
                    DecalShape::Ring,
                ))
            });
        },
        move |fx| fx.release_decal(d2),
    );
}

/// Éclat lumineux (sprite additif) qui grossit puis s'éteint.
pub fn flash(fx: &mut Fx, pos: Vec3, size: f32, color: Vec3, duration: f32) {
    let sp = fx.acquire_sprite(color, size * 0.3);
    set_sprite(fx, &sp, pos, size * 0.3, color, 1.0);
    let sp2 = sp.clone();
    timed(
        fx,
        duration,
        move |fx, t, _| {
            let s = size * (0.3 + 0.7 * ease::out_cubic(t));
            set_sprite(fx, &sp, pos, s, color, 1.0 - t);
        },
        move |fx| fx.release_sprite(sp2),
    );
}

/// Émission continue de particules dans une zone.
#[derive(Clone, Copy, Debug)]
pub struct ZoneEmit {
    pub style: ParticleStyle,
    /// Particules par seconde.
    pub rate: f32,
    pub size: [f32; 2],
    pub life: [f32; 2],
    pub up: Option<[f32; 2]>,
    pub tint: Option<Vec3>,
    pub intensity: f32,
}

/// Options d'une zone persistante (`ZoneOptions`).
#[derive(Clone, Copy, Debug, Default)]
pub struct ZoneOptions {
    pub fill: Option<f32>,
    pub pattern: Option<f32>,
    pub emit: Option<ZoneEmit>,
    pub normal_blend: bool,
}

/// Zone persistante (blizzard, nuée, aura) : fondu d'entrée et de sortie, motif animé, particules.
pub fn zone(fx: &mut Fx, at: Vec3, radius: f32, duration: f32, color: Vec3, o: ZoneOptions) {
    let d = fx.acquire_decal();
    let ground = fx.ground_color(color);
    edit_decal(fx, &d, |m, g| {
        m.set_color(ground, ground);
        m.u.fill = o.fill.unwrap_or(0.18);
        m.u.pattern = o.pattern.unwrap_or(0.6);
        m.u.edge = 0.1;
        if o.normal_blend {
            m.set_blend(Blend::Normal);
        }
        Some(g.circle(m, at, radius, DecalShape::Circle))
    });
    let d2 = d.clone();
    let mut acc = 0.0;
    timed(
        fx,
        duration,
        move |fx, t, dt| {
            let fade_in = (t * duration * 5.0).min(1.0);
            let fade_out = ((1.0 - t) * duration * 4.0).min(1.0);
            decal_opacity(fx, &d, fade_in.min(fade_out));
            if let Some(e) = o.emit {
                acc += e.rate * dt;
                let n = acc.floor();
                if n > 0.0 {
                    acc -= n;
                    fx.ps().emit(
                        e.style,
                        at,
                        &EmitOptions {
                            count: n,
                            shape: EmitShape::Disc,
                            radius: radius * 0.95,
                            life: e.life.into(),
                            size: e.size.into(),
                            up: e.up.map(Range::from),
                            tint: e.tint.unwrap_or(color),
                            intensity: e.intensity,
                            ..default()
                        },
                    );
                }
            }
        },
        move |fx| fx.release_decal(d2),
    );
}

/// Suit une entité pendant `duration` : `step(fx, pos, t, dt)` reçoit sa position monde au sol.
pub fn follow(
    fx: &mut Fx,
    entity: EntityId,
    duration: f32,
    mut step: impl FnMut(&mut Fx, Vec3, f32, f32) + Send + Sync + 'static,
    done: impl FnOnce(&mut Fx) + Send + Sync + 'static,
) {
    timed(
        fx,
        duration,
        move |fx, t, dt| {
            if let Some(p) = fx.entity_pos(entity, 0.0) {
                step(fx, p, t, dt);
            }
        },
        done,
    );
}

// ---------------------------------------------------------------------------------- projectile

/// Appelé à chaque image avec la tête et la direction du projectile.
pub type ProjectileStep = Box<dyn FnMut(&mut Fx, Vec3, Vec3, f32) + Send + Sync>;
/// Appelé à l'arrivée du projectile, avec la position de sa tête.
pub type ProjectileEnd = Box<dyn FnOnce(&mut Fx, Vec3) + Send + Sync>;

/// Apparence d'un projectile (`ProjectileLook`).
pub struct ProjectileLook {
    pub color: Vec3,
    pub core: Option<Vec3>,
    /// Taille du halo de tête (monde) ; 0 = pas de halo.
    pub size: f32,
    pub height: f32,
    /// Particules de traînée par unité monde parcourue.
    pub trail_density: f32,
    pub trail_style: Option<ParticleStyle>,
    pub trail_size: [f32; 2],
    pub trail_life: [f32; 2],
    /// Largeur du ruban ; 0 = pas de ruban.
    pub ribbon_width: f32,
    pub ribbon_life: f32,
    pub on_step: Option<ProjectileStep>,
    pub on_end: Option<ProjectileEnd>,
    /// Tête chercheuse : suit la position courante de cette entité (auto-attaques, sorts ciblés).
    pub target: Option<EntityId>,
}

impl Default for ProjectileLook {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            core: None,
            size: 0.6,
            height: 1.0,
            trail_density: 6.0,
            trail_style: None,
            trail_size: [0.15, 0.3],
            trail_life: [0.25, 0.45],
            ribbon_width: 0.25,
            ribbon_life: 0.25,
            on_step: None,
            on_end: None,
            target: None,
        }
    }
}

/// Demande d'arrêt d'un projectile, partagée entre l'effet et sa poignée.
#[derive(Default)]
struct Landing {
    requested: bool,
    at: Option<Vec3>,
}

/// Poignée d'un [`ProjectileFx`] : l'impact serveur l'arrête là où il a touché.
pub struct ProjectileStop(Arc<Mutex<Landing>>);

impl ProjectileStop {
    /// Termine le vol (en plaçant la tête sur `at` si fourni).
    pub fn stop(&self, at: Option<Vec3>) {
        if let Ok(mut l) = self.0.lock() {
            l.requested = true;
            l.at = at;
        }
    }
}

impl ProjectileHandle for ProjectileStop {
    fn finish(&mut self, _fx: &mut Fx, at: Vec3) {
        self.stop(Some(at));
    }
}

/// Projectile visuel : va de `from` à `to` à `speed` (monde/s), ou jusqu'à l'arrêt par sa poignée.
pub struct ProjectileFx {
    head: Vec3,
    dir: Vec3,
    from: Vec3,
    dist: f32,
    traveled: f32,
    speed: f32,
    flying: bool,
    acc: f32,
    sprite: FxSprite,
    ribbon: Option<FxRibbon>,
    look: ProjectileLook,
    landing: Arc<Mutex<Landing>>,
}

impl ProjectileFx {
    /// Lance le projectile et renvoie sa poignée (à rendre au dispatcher pour l'impact serveur).
    pub fn launch(
        fx: &mut Fx,
        from: Vec3,
        to: Vec3,
        speed: f32,
        look: ProjectileLook,
    ) -> ProjectileStop {
        let h = look.height;
        let from = Vec3::new(from.x, h, from.z);
        let mut dir = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
        let dist = dir.length();
        if dist > 1e-4 {
            dir /= dist;
        } else {
            dir = Vec3::X;
        }
        let head_color = look.core.unwrap_or(look.color);
        let sprite = fx.acquire_sprite(head_color, look.size);
        set_sprite(fx, &sprite, from, look.size, head_color, 1.0);
        if look.size <= 0.0 {
            sprite_visible(fx, &sprite, false);
        }
        let ribbon = (look.ribbon_width > 0.0).then(|| {
            let r = fx.acquire_ribbon(look.color);
            let (w, l) = (look.ribbon_width, look.ribbon_life);
            ribbon_setup(fx, &r, |rb| {
                rb.width = w;
                rb.life = l;
            });
            ribbon_color(fx, &r, look.color, look.core);
            push_ribbon(fx, &r, from);
            r
        });
        let landing = Arc::new(Mutex::new(Landing::default()));
        fx.add(ProjectileFx {
            head: from,
            dir,
            from,
            dist,
            traveled: 0.0,
            speed,
            flying: true,
            acc: 0.0,
            sprite,
            ribbon,
            look,
            landing: landing.clone(),
        });
        ProjectileStop(landing)
    }

    fn land(&mut self, fx: &mut Fx) {
        self.flying = false;
        sprite_visible(fx, &self.sprite, false);
        if let Some(r) = &self.ribbon {
            push_ribbon(fx, r, self.head);
        }
        if let Some(end) = self.look.on_end.take() {
            end(fx, self.head);
        }
    }
}

impl Effect for ProjectileFx {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        if self.flying {
            let stop = self
                .landing
                .lock()
                .ok()
                .filter(|l| l.requested)
                .map(|l| l.at);
            if let Some(at) = stop {
                if let Some(at) = at {
                    self.head = Vec3::new(at.x, self.head.y, at.z);
                }
                self.land(fx);
                return self
                    .ribbon
                    .as_ref()
                    .is_some_and(|r| ribbon_alive(fx, r) > 0);
            }
            let prev = self.head;
            let aim = self
                .look
                .target
                .and_then(|id| fx.entity_pos(id, self.head.y));
            if let Some(aim) = aim {
                // tête chercheuse : avance vers la position actuelle de la cible
                let to = aim - self.head;
                let d = to.length();
                let step = self.speed * dt;
                if d > 1e-4 {
                    self.dir = to / d;
                }
                self.head += self.dir * step.min(d);
                // arrivé quand le pas couvre la distance restante ; sinon on reste en vol même si la
                // cible s'éloigne
                self.traveled = if step >= d {
                    self.dist
                } else {
                    (self.dist - 1e-3).min(self.traveled + step)
                };
            } else {
                self.traveled = self.dist.min(self.traveled + self.speed * dt);
                self.head = self.from + self.dir * self.traveled;
            }
            let color = self.look.core.unwrap_or(self.look.color);
            set_sprite(fx, &self.sprite, self.head, self.look.size, color, 1.0);
            if let Some(r) = &self.ribbon {
                push_ribbon(fx, r, self.head);
            }
            if let Some(step) = self.look.on_step.as_mut() {
                step(fx, self.head, self.dir, dt);
            }
            if let Some(style) = self.look.trail_style {
                self.acc += prev.distance(self.head) * self.look.trail_density;
                let n = self.acc.floor();
                if n > 0.0 {
                    self.acc -= n;
                    let inherit = self.dir * (-self.speed * 0.08);
                    let (life, size, tint, head) = (
                        self.look.trail_life,
                        self.look.trail_size,
                        self.look.color,
                        self.head,
                    );
                    fx.ps().emit(
                        style,
                        prev,
                        &EmitOptions {
                            count: n,
                            shape: EmitShape::Line,
                            to: Some(head),
                            radius: 0.1,
                            speed: between(0.1, 0.5),
                            inherit: Some(inherit),
                            life: life.into(),
                            size: size.into(),
                            tint,
                            ..default()
                        },
                    );
                }
            }
            if self.traveled >= self.dist {
                self.land(fx);
            }
            return true;
        }
        self.ribbon
            .as_ref()
            .is_some_and(|r| ribbon_alive(fx, r) > 0)
    }

    fn end(&mut self, fx: &mut Fx) {
        fx.release_sprite(self.sprite.clone());
        if let Some(r) = self.ribbon.take() {
            fx.release_ribbon(r);
        }
    }
}

// ---------------------------------------------------------------------------------- dash, éclair

/// Traînée au sol et poussière pendant un dash (suit l'entité si connue, sinon interpole `from` → `to`).
pub fn dash_trail(
    fx: &mut Fx,
    entity: EntityId,
    from: Vec3,
    to: Vec3,
    duration: f32,
    color: Vec3,
    dust: bool,
) {
    let r = fx.acquire_ribbon(color);
    ribbon_setup(fx, &r, |rb| {
        rb.mode = RibbonMode::Flat;
        rb.width = 0.7;
        rb.life = 0.4;
    });
    fx.add(DashTrail {
        ribbon: r,
        entity,
        from,
        to,
        prev: from,
        duration,
        elapsed: 0.0,
        flying: true,
        dust,
    });
}

struct DashTrail {
    ribbon: FxRibbon,
    entity: EntityId,
    from: Vec3,
    to: Vec3,
    prev: Vec3,
    duration: f32,
    elapsed: f32,
    flying: bool,
    dust: bool,
}

impl Effect for DashTrail {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        if !self.flying {
            return ribbon_alive(fx, &self.ribbon) > 0;
        }
        self.elapsed += dt;
        let t = (self.elapsed / self.duration.max(0.05)).min(1.0);
        let mut p = fx
            .entity_pos(self.entity, 0.0)
            .unwrap_or_else(|| self.from.lerp(self.to, t));
        p.y = 0.25;
        push_ribbon(fx, &self.ribbon, p);
        if self.dust && self.prev.distance_squared(p) > 0.04 {
            let s = fx.styles();
            fx.ps().emit(
                s.dust,
                p,
                &EmitOptions {
                    count: 2.0,
                    shape: EmitShape::Disc,
                    radius: 0.25,
                    life: [0.4, 0.7].into(),
                    size: [0.4, 0.8].into(),
                    up: between(0.2, 0.6),
                    ..default()
                },
            );
            self.prev = p;
        }
        if t >= 1.0 {
            self.flying = false;
        }
        true
    }

    fn end(&mut self, fx: &mut Fx) {
        fx.release_ribbon(self.ribbon.clone());
    }
}

/// Réglages d'un éclair.
#[derive(Clone, Copy, Debug)]
pub struct LightningOptions {
    pub width: f32,
    pub life: f32,
    pub jag: f32,
    pub segments: u32,
}

impl Default for LightningOptions {
    fn default() -> Self {
        Self {
            width: 0.18,
            life: 0.25,
            jag: 0.25,
            segments: 10,
        }
    }
}

/// Éclair en zigzag (ruban) de `from` à `to`, qui s'efface en `life` s.
pub fn lightning(fx: &mut Fx, from: Vec3, to: Vec3, color: Vec3, o: LightningOptions) {
    let r = fx.acquire_ribbon(color);
    ribbon_setup(fx, &r, |rb| {
        rb.width = o.width;
        rb.life = o.life;
        rb.min_dist = 0.001;
    });
    ribbon_color(fx, &r, color, Some(Vec3::splat(3.0)));
    let n = o.segments.max(1);
    for i in 0..=n {
        let k = i as f32 / n as f32;
        let mut p = from.lerp(to, k);
        if i > 0 && i < n {
            p += Vec3::new(
                (rand() - 0.5) * o.jag * 2.0,
                (rand() - 0.5) * o.jag,
                (rand() - 0.5) * o.jag * 2.0,
            );
        }
        push_ribbon(fx, &r, p);
    }
    fx.add(Fading(r));
}

/// Ruban laissé à lui-même jusqu'à ce qu'il s'efface.
struct Fading(FxRibbon);

impl Effect for Fading {
    fn update(&mut self, fx: &mut Fx, _: f32) -> bool {
        ribbon_alive(fx, &self.0) > 0
    }

    fn end(&mut self, fx: &mut Fx) {
        fx.release_ribbon(self.0.clone());
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::fx::core::runtime::{FxRuntime, SpriteAssets};
    use crate::fx::particles::{CAPACITY, ParticleSystem, StdStyles};
    use crate::fx::shaders::DecalMesh;

    /// Monde minimal où les primitives peuvent tourner (pools, particules, assets), sans rendu.
    pub(crate) fn fx_world() -> World {
        let mut w = World::new();
        w.init_resource::<FxRuntime>();
        w.init_resource::<Assets<Mesh>>();
        w.init_resource::<Assets<Image>>();
        w.init_resource::<Assets<DecalMaterial>>();
        w.init_resource::<Assets<RibbonMaterial>>();
        w.init_resource::<Assets<StandardMaterial>>();
        w.init_resource::<Assets<crate::fx::shaders::FresnelMaterial>>();
        w.init_resource::<Assets<crate::fx::shaders::IceMaterial>>();
        w.init_resource::<DecalMesh>();
        w.init_resource::<SpriteAssets>();
        let mut ps = ParticleSystem::new(CAPACITY, Rng::new(1));
        let styles = StdStyles::create(&mut ps);
        w.insert_resource(ps);
        w.insert_resource(styles);
        w
    }

    fn run(w: &mut World, seconds: f32) {
        let mut t = 0.0;
        while t < seconds {
            FxRuntime::update(w, 1.0 / 30.0);
            t += 1.0 / 30.0;
        }
    }

    /// Les rubans vieillissent dans leur propre système (absent ici) : on les vide à la main.
    fn age_ribbons(w: &mut World) {
        let mut q = w.query::<&mut Ribbon>();
        for mut r in q.iter_mut(w) {
            r.update(1.0, Vec3::Y);
        }
    }

    fn visible_decals(w: &mut World) -> usize {
        w.query::<(&GroundDecal, &Visibility)>()
            .iter(w)
            .filter(|(_, v)| **v != Visibility::Hidden)
            .count()
    }

    #[test]
    fn telegraphie_remplie_puis_rendue_au_pool() {
        static DONE: AtomicU32 = AtomicU32::new(0);
        let mut w = fx_world();
        telegraph_then(
            &mut Fx::new(&mut w),
            Vec3::ZERO,
            2.0,
            0.5,
            Vec3::ONE,
            TelegraphOptions::default(),
            |_| {
                DONE.fetch_add(1, Ordering::Relaxed);
            },
        );
        assert_eq!(visible_decals(&mut w), 1);
        run(&mut w, 0.3);
        assert_eq!(DONE.load(Ordering::Relaxed), 0);
        run(&mut w, 0.3);
        assert_eq!(
            DONE.load(Ordering::Relaxed),
            1,
            "explosion à la fin du remplissage"
        );
        run(&mut w, 0.3);
        assert_eq!(visible_decals(&mut w), 0, "décal rendu après le fondu");
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 0);
    }

    #[test]
    fn onde_eclat_et_zone_se_terminent() {
        let mut w = fx_world();
        let mut fx = Fx::new(&mut w);
        shockwave(&mut fx, Vec3::ZERO, 2.0, 0.3, Vec3::ONE, 0.12);
        flash(&mut fx, Vec3::Y, 1.0, Vec3::ONE, 0.25);
        let s = fx.styles();
        zone(
            &mut fx,
            Vec3::ZERO,
            3.0,
            0.6,
            Vec3::ONE,
            ZoneOptions {
                emit: Some(ZoneEmit {
                    style: s.glow,
                    rate: 60.0,
                    size: [0.2, 0.3],
                    life: [0.3, 0.5],
                    up: None,
                    tint: None,
                    intensity: 1.0,
                }),
                ..default()
            },
        );
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 3);
        run(&mut w, 1.0);
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 0);
        assert!(w.resource::<ParticleSystem>().emitted > 10, "la zone émet");
        assert_eq!(visible_decals(&mut w), 0);
    }

    #[test]
    fn projectile_arrete_par_l_impact_serveur() {
        static ENDS: AtomicU32 = AtomicU32::new(0);
        let mut w = fx_world();
        let mut stop = ProjectileFx::launch(
            &mut Fx::new(&mut w),
            Vec3::ZERO,
            Vec3::new(100.0, 0.0, 0.0),
            10.0,
            ProjectileLook {
                on_end: Some(Box::new(|_, at| {
                    assert!((at.x - 3.0).abs() < 1e-4, "posé sur l'impact");
                    ENDS.fetch_add(1, Ordering::Relaxed);
                })),
                ..default()
            },
        );
        run(&mut w, 0.2);
        assert_eq!(ENDS.load(Ordering::Relaxed), 0, "toujours en vol");
        stop.finish(&mut Fx::new(&mut w), Vec3::new(3.0, 0.0, 0.0));
        run(&mut w, 0.1);
        assert_eq!(ENDS.load(Ordering::Relaxed), 1);
        age_ribbons(&mut w);
        run(&mut w, 0.1);
        assert_eq!(
            w.resource::<FxRuntime>().active_effects(),
            0,
            "ruban effacé"
        );
    }

    #[test]
    fn projectile_arrive_seul_a_destination() {
        let mut w = fx_world();
        let _ = ProjectileFx::launch(
            &mut Fx::new(&mut w),
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            10.0,
            ProjectileLook::default(),
        );
        run(&mut w, 0.5);
        age_ribbons(&mut w);
        run(&mut w, 0.1);
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 0);
    }

    #[test]
    fn dash_et_eclair_s_effacent() {
        let mut w = fx_world();
        let mut fx = Fx::new(&mut w);
        dash_trail(&mut fx, 9, Vec3::ZERO, Vec3::X * 4.0, 0.2, Vec3::ONE, true);
        lightning(
            &mut fx,
            Vec3::ZERO,
            Vec3::X * 4.0,
            Vec3::ONE,
            LightningOptions::default(),
        );
        assert_eq!(fx.active_effects(), 2);
        run(&mut w, 0.3);
        age_ribbons(&mut w);
        run(&mut w, 0.1);
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 0);
    }
}
