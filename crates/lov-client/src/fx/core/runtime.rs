//! Runtime FX : effets actifs, minuteur, couleurs, auras et pools (décals, rubans, sprites). Les handlers
//! de sorts (`register_fx`) reçoivent un contexte [`Fx`] et composent ses primitives.
//!
//! Portage de `packages/client/src/fx/core/FxRuntime.ts`. Le web garde dans le runtime des objets three.js
//! qu'il modifie directement ; en Bevy, décals, rubans et sprites sont des entités, et les particules une
//! ressource. Le runtime ([`FxRuntime`]) ne garde donc que l'état des FX, et [`Fx`] donne aux handlers et aux
//! effets l'accès au monde, dans le système exclusif qui fait tourner les FX.
//!
//! Écarts avec le web :
//! - les systèmes de meshes instanciés (`bursts`, `effects/meshBursts.ts`) relèvent de l'area fx-effects ;
//! - le tremblement de caméra est publié en message [`CameraShake`] : la caméra ne l'applique pas encore ;
//! - les sprites ne désactivent pas le test de profondeur (`depthTest: false` du web) : le matériau
//!   standard de Bevy ne le permet pas ;
//! - un effet ajouté par une tâche du minuteur fait sa première mise à jour à l'image suivante.

use std::collections::HashMap;

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use lov_shared::types::{EntityId, EntitySnapshot, Vec2 as GameVec2};

use super::entities::EntityTracker;
use super::palette::{FxColors, palette_for_key};
use super::scheduler::Scheduler;
use crate::fx::particles::{ParticleSystem, StdStyles};
use crate::fx::shaders::{
    DecalMaterial, DecalMesh, GroundDecal, Ribbon, RibbonMaterial, RibbonMode,
};
use crate::render::RENDER_SCALE;
use crate::render::camera::MainCamera;

/// Effet actif : `update` renvoie `false` quand il est terminé (le runtime appelle alors `end`).
pub trait Effect: Send + Sync + 'static {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool;
    fn end(&mut self, _fx: &mut Fx) {}
}

/// Annulation d'un effet persistant (bouclier, aura).
type AuraCancel = Box<dyn FnOnce(&mut Fx) + Send + Sync>;

/// Identifiant d'une aura posée par [`Fx::set_aura`], pour [`Fx::forget_aura`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuraId(u64);

/// Tremblement de caméra demandé par un impact (amplitude en unités de jeu, durée en s).
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct CameraShake {
    pub intensity: f32,
    pub duration: f32,
}

/// Nombre maximal de passes de [`FxRuntime::reset`] : chaque passe termine les effets ajoutés par les
/// `end` de la précédente.
const RESET_ROUNDS: usize = 8;

/// Teinte vers laquelle tirent les zones au sol d'un ennemi.
const ENEMY_TINT: Vec3 = Vec3::new(2.4, 0.25, 0.2);

/// Décal au sol sorti du pool : entité, son matériau (couleurs, forme, opacité) et son placement.
#[derive(Debug, Clone)]
pub struct FxDecal {
    pub entity: Entity,
    pub material: Handle<DecalMaterial>,
    pub decal: GroundDecal,
}

/// Ruban de traînée sorti du pool : entité (composant [`Ribbon`]) et son matériau.
#[derive(Debug, Clone)]
pub struct FxRibbon {
    pub entity: Entity,
    pub material: Handle<RibbonMaterial>,
}

/// Éclat lumineux face caméra sorti du pool (`THREE.Sprite` additif du web).
#[derive(Debug, Clone)]
pub struct FxSprite {
    pub entity: Entity,
    pub material: Handle<StandardMaterial>,
}

/// Rotation d'un sprite autour de l'axe de vue (`material.rotation` du web) ; il fait face à la caméra.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Billboard {
    pub rotation: f32,
}

/// Maillage et texture partagés des sprites.
#[derive(Resource)]
pub struct SpriteAssets {
    pub mesh: Handle<Mesh>,
    pub glow: Handle<Image>,
}

impl FromWorld for SpriteAssets {
    fn from_world(world: &mut World) -> Self {
        use crate::fx::particles::atlas::{CELL, Tex, cell_rgba};
        use bevy::asset::RenderAssetUsages;
        use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
        let mut image = Image::new(
            Extent3d {
                width: CELL as u32,
                height: CELL as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            cell_rgba(Tex::Glow),
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = bevy::image::ImageSampler::linear();
        let glow = world.resource_mut::<Assets<Image>>().add(image);
        let mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Rectangle::new(1.0, 1.0));
        Self { mesh, glow }
    }
}

/// État du runtime FX (`FxRuntime`).
#[derive(Resource, Default)]
pub struct FxRuntime {
    /// Temps des FX (s), avancé à chaque image.
    pub time: f32,
    /// Vrai pendant l'exécution d'un handler dont l'auteur est un ennemi du joueur local (posé par le
    /// dispatcher). Les primitives au sol (télégraphies, zones) teintent alors leur couleur vers le rouge :
    /// en teamfight, on distingue d'un coup d'œil les zones dangereuses.
    pub enemy_context: bool,
    effects: Vec<Box<dyn Effect>>,
    scheduler: Scheduler<World>,
    colors: HashMap<String, FxColors>,
    auras: HashMap<String, (AuraId, AuraCancel)>,
    next_aura: u64,
    free_decals: Vec<FxDecal>,
    free_ribbons: Vec<FxRibbon>,
    free_sprites: Vec<FxSprite>,
}

impl FxRuntime {
    /// Couleurs de la palette associée à une fxKey (mises en cache).
    pub fn colors(&mut self, key: &str) -> FxColors {
        if let Some(c) = self.colors.get(key) {
            return *c;
        }
        let c = FxColors::of(palette_for_key(key));
        self.colors.insert(key.to_owned(), c);
        c
    }

    /// Couleur de zone au sol : teintée vers le rouge si l'auteur est ennemi (voir `enemy_context`).
    pub fn ground_color(&self, c: Vec3) -> Vec3 {
        if self.enemy_context {
            c.lerp(ENEMY_TINT, 0.45)
        } else {
            c
        }
    }

    pub fn active_effects(&self) -> usize {
        self.effects.len()
    }

    pub fn pending_tasks(&self) -> usize {
        self.scheduler.pending()
    }

    /// Termine tous les effets en cours (leurs entités retournent aux pools) et vide le minuteur et les
    /// auras : entre deux parties, rien ne doit survivre (le web recréait le runtime à chaque `installFx`).
    ///
    /// Un `end` peut lui-même ajouter un effet qui détient un élément d'un pool (la fin d'une télégraphie
    /// lance le fondu qui rend son décal, la fin d'une invisibilité lance un éclat) : on termine donc en
    /// boucle, tant que de nouveaux effets apparaissent, avant de vider le minuteur et les auras.
    pub fn reset(world: &mut World) {
        world.resource_mut::<FxRuntime>().enemy_context = false;
        for _ in 0..RESET_ROUNDS {
            let effects = std::mem::take(&mut world.resource_mut::<FxRuntime>().effects);
            if effects.is_empty() {
                break;
            }
            let mut fx = Fx::new(world);
            for mut e in effects {
                e.end(&mut fx);
            }
        }
        let mut rt = world.resource_mut::<FxRuntime>();
        // au-delà de la borne (effets qui se relancent sans fin), on jette ce qui reste
        rt.effects.clear();
        rt.scheduler.clear();
        rt.auras.clear();
    }

    /// Avance le temps, le minuteur et les effets (`update` du web, hors particules et rubans qui ont leurs
    /// propres systèmes).
    pub fn update(world: &mut World, dt: f32) {
        let (mut scheduler, mut effects) = {
            let mut rt = world.resource_mut::<FxRuntime>();
            rt.time += dt;
            (
                std::mem::take(&mut rt.scheduler),
                std::mem::take(&mut rt.effects),
            )
        };
        scheduler.update(dt, world);
        let mut fx = Fx::new(world);
        let mut i = effects.len();
        while i > 0 {
            i -= 1;
            if effects[i].update(&mut fx, dt) {
                continue;
            }
            let mut done = effects.swap_remove(i);
            done.end(&mut fx);
        }
        let mut rt = world.resource_mut::<FxRuntime>();
        // ce qui a été planifié ou ajouté pendant la mise à jour est allé dans le runtime
        let added = std::mem::take(&mut rt.scheduler);
        scheduler.absorb(added);
        rt.scheduler = scheduler;
        effects.append(&mut rt.effects);
        rt.effects = effects;
    }
}

/// Contexte des handlers et des effets : le runtime et le monde Bevy.
pub struct Fx<'w> {
    world: &'w mut World,
}

impl<'w> Fx<'w> {
    pub fn new(world: &'w mut World) -> Self {
        Self { world }
    }

    /// Échappatoire : accès direct au monde.
    pub fn world(&mut self) -> &mut World {
        self.world
    }

    pub fn rt(&mut self) -> Mut<'_, FxRuntime> {
        self.world.resource_mut::<FxRuntime>()
    }

    pub fn time(&self) -> f32 {
        self.world.resource::<FxRuntime>().time
    }

    pub fn enemy_context(&self) -> bool {
        self.world.resource::<FxRuntime>().enemy_context
    }

    /// Particules (ressource du socle FX).
    pub fn ps(&mut self) -> Mut<'_, ParticleSystem> {
        self.world.resource_mut::<ParticleSystem>()
    }

    /// Styles de particules standard.
    pub fn styles(&self) -> StdStyles {
        *self.world.resource::<StdStyles>()
    }

    /// Ajoute un effet actif.
    pub fn add(&mut self, e: impl Effect) {
        self.rt().effects.push(Box::new(e));
    }

    /// Exécute `f` dans `delay` secondes (temps des FX).
    pub fn after(&mut self, delay: f32, f: impl FnOnce(&mut Fx) + Send + Sync + 'static) {
        self.rt()
            .scheduler
            .after(delay, move |_, world| f(&mut Fx::new(world)));
    }

    pub fn colors(&mut self, key: &str) -> FxColors {
        self.rt().colors(key)
    }

    pub fn ground_color(&self, c: Vec3) -> Vec3 {
        self.world.resource::<FxRuntime>().ground_color(c)
    }

    pub fn active_effects(&self) -> usize {
        self.world.resource::<FxRuntime>().active_effects()
    }

    /// Coordonnées de jeu → monde, posées sur le relief du terrain (rivière creusée), `h` au-dessus du
    /// sol : télégraphes, impacts et zones suivent le lit de la rivière comme les unités (#274).
    pub fn world_pos(p: GameVec2, h: f32) -> Vec3 {
        crate::render::to_ground(bevy::math::Vec2::new(p.x as f32, p.y as f32), h)
    }

    /// Distance de jeu → monde.
    pub fn scale(game_units: f32) -> f32 {
        game_units * RENDER_SCALE
    }

    pub fn entities(&self) -> EntityTracker<'_> {
        EntityTracker::of(self.world)
    }

    /// Position monde de l'entité (+ hauteur `h`), si elle est connue.
    pub fn entity_pos(&self, id: EntityId, h: f32) -> Option<Vec3> {
        self.entities().pos(id, h)
    }

    pub fn entity(&self, id: EntityId) -> Option<&EntitySnapshot> {
        EntityTracker::of(self.world).info(id)
    }

    pub fn is_enemy(&self, id: EntityId) -> bool {
        self.entities().is_enemy(id)
    }

    /// Position de la caméra de la partie.
    pub fn camera_pos(&mut self) -> Option<Vec3> {
        let mut q = self
            .world
            .query_filtered::<&GlobalTransform, With<MainCamera>>();
        q.iter(self.world).next().map(GlobalTransform::translation)
    }

    /// Tremblement de caméra pour un impact en `at` (monde), atténué avec la distance à la caméra : une
    /// explosion hors écran ne secoue pas la vue. `intensity` en unités de jeu (15 léger, 40 fort).
    pub fn shake(&mut self, at: Vec3, intensity: f32, duration: f32) {
        let Some(cam) = self.camera_pos() else { return };
        // point visé ≈ projection de la caméra au sol (vue plongeante)
        let d = (at.x - cam.x).hypot(at.z - (cam.z - cam.y * 0.6));
        let k = 1.0 - d / 30.0;
        if k > 0.0 {
            self.world.write_message(CameraShake {
                intensity: intensity * k,
                duration,
            });
        }
    }

    // ------------------------------------------------------------------------------------------ auras

    /// Enregistre l'annulation d'un effet persistant (bouclier, aura) ; remplace celui de même clé.
    pub fn set_aura(
        &mut self,
        key: &str,
        cancel: impl FnOnce(&mut Fx) + Send + Sync + 'static,
    ) -> AuraId {
        let (id, old) = {
            let mut rt = self.rt();
            let id = AuraId(rt.next_aura);
            rt.next_aura += 1;
            let old = rt.auras.insert(key.to_owned(), (id, Box::new(cancel)));
            (id, old)
        };
        if let Some((_, c)) = old {
            c(self);
        }
        id
    }

    /// Termine l'effet persistant `key` (ex. sur `buffRemoved`).
    pub fn end_aura(&mut self, key: &str) {
        let c = self.rt().auras.remove(key);
        if let Some((_, c)) = c {
            c(self);
        }
    }

    /// À appeler par l'effet lui-même quand il se termine seul.
    pub fn forget_aura(&mut self, key: &str, id: AuraId) {
        let mut rt = self.rt();
        if rt.auras.get(key).is_some_and(|(cur, _)| *cur == id) {
            rt.auras.remove(key);
        }
    }

    // ------------------------------------------------------------------------------------------ pools

    /// Décal au sol remis à zéro et visible ; le placer avec `decal.circle`/`rect`/`cone` sur son matériau.
    pub fn acquire_decal(&mut self) -> FxDecal {
        let d = self.rt().free_decals.pop();
        let d = d.unwrap_or_else(|| {
            let decal = GroundDecal::default();
            let material = self
                .world
                .resource_mut::<Assets<DecalMaterial>>()
                .add(DecalMaterial::default());
            let bundle = self.world.resource::<DecalMesh>().bundle(
                decal,
                material.clone(),
                Transform::default(),
            );
            // `Visibility` explicite : le pool cache les décals rendus
            let entity = self.world.spawn((bundle, Visibility::Hidden)).id();
            FxDecal {
                entity,
                material,
                decal,
            }
        });
        if let Some(mut m) = self
            .world
            .resource_mut::<Assets<DecalMaterial>>()
            .get_mut(&d.material)
        {
            m.reset();
        }
        self.set_visible(d.entity, true);
        d
    }

    pub fn release_decal(&mut self, d: FxDecal) {
        self.set_visible(d.entity, false);
        self.rt().free_decals.push(d);
    }

    /// Ruban vidé, visible, aux réglages par défaut du web (largeur 0,3, vie 0,35 s, face caméra).
    pub fn acquire_ribbon(&mut self, color: Vec3) -> FxRibbon {
        let r = self.rt().free_ribbons.pop();
        let r = r.unwrap_or_else(|| {
            let ribbon = Ribbon::default();
            let mesh = self.world.resource_mut::<Assets<Mesh>>().add(ribbon.mesh());
            let material = self
                .world
                .resource_mut::<Assets<RibbonMaterial>>()
                .add(RibbonMaterial::new(color, None));
            let entity = self.world.spawn(ribbon.bundle(mesh, material.clone())).id();
            FxRibbon { entity, material }
        });
        if let Some(mut m) = self
            .world
            .resource_mut::<Assets<RibbonMaterial>>()
            .get_mut(&r.material)
        {
            *m = RibbonMaterial::new(color, None);
        }
        if let Some(mut ribbon) = self.world.get_mut::<Ribbon>(r.entity) {
            ribbon.reset();
            ribbon.width = 0.3;
            ribbon.life = 0.35;
            ribbon.min_dist = 0.08;
            ribbon.mode = RibbonMode::Billboard;
        }
        r
    }

    pub fn release_ribbon(&mut self, r: FxRibbon) {
        if let Some(mut ribbon) = self.world.get_mut::<Ribbon>(r.entity) {
            ribbon.reset();
        }
        self.set_visible(r.entity, false);
        self.rt().free_ribbons.push(r);
    }

    /// Éclat additif face caméra, de couleur `color` (linéaire, HDR permis) et de taille `size` (monde).
    pub fn acquire_sprite(&mut self, color: Vec3, size: f32) -> FxSprite {
        let sp = self.rt().free_sprites.pop();
        let sp = sp.unwrap_or_else(|| {
            let (mesh, glow) = {
                let a = self.world.resource::<SpriteAssets>();
                (a.mesh.clone(), a.glow.clone())
            };
            let material =
                self.world
                    .resource_mut::<Assets<StandardMaterial>>()
                    .add(StandardMaterial {
                        base_color_texture: Some(glow),
                        unlit: true,
                        alpha_mode: AlphaMode::Add,
                        // pas de `depth_bias` : sur `StandardMaterial`, c'est un vrai biais de profondeur
                        // du pipeline, pas une simple clé de tri ; le mélange additif ne dépend pas de
                        // l'ordre de dessin
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    });
            let entity = self
                .world
                .spawn((
                    Name::new("fx-sprite"),
                    Mesh3d(mesh),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    Billboard::default(),
                    NoFrustumCulling,
                ))
                .id();
            FxSprite { entity, material }
        });
        if let Some(mut m) = self
            .world
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&sp.material)
        {
            m.base_color = Color::LinearRgba(LinearRgba::rgb(color.x, color.y, color.z));
        }
        if let Some(mut tf) = self.world.get_mut::<Transform>(sp.entity) {
            tf.scale = Vec3::new(size, size, 1.0);
        }
        if let Some(mut b) = self.world.get_mut::<Billboard>(sp.entity) {
            b.rotation = 0.0;
        }
        self.set_visible(sp.entity, true);
        sp
    }

    pub fn release_sprite(&mut self, sp: FxSprite) {
        self.set_visible(sp.entity, false);
        self.rt().free_sprites.push(sp);
    }

    fn set_visible(&mut self, entity: Entity, on: bool) {
        if let Some(mut v) = self.world.get_mut::<Visibility>(entity) {
            v.set_if_neq(if on {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
}

/// Oriente les sprites face à la caméra de la partie.
pub fn face_camera(
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut sprites: Query<(&Billboard, &mut Transform)>,
) {
    let Some(cam) = camera.iter().next() else {
        return;
    };
    let (_, rot, _) = cam.to_scale_rotation_translation();
    for (b, mut tf) in &mut sprites {
        tf.rotation = rot * Quat::from_rotation_z(b.rotation);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    fn world() -> World {
        let mut w = World::new();
        w.init_resource::<FxRuntime>();
        w
    }

    struct Countdown {
        left: u32,
        ended: Arc<AtomicU32>,
    }

    impl Effect for Countdown {
        fn update(&mut self, _fx: &mut Fx, _dt: f32) -> bool {
            self.left = self.left.saturating_sub(1);
            self.left > 0
        }
        fn end(&mut self, _fx: &mut Fx) {
            self.ended.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn les_effets_tournent_puis_se_terminent() {
        let mut w = world();
        let ended = Arc::new(AtomicU32::new(0));
        Fx::new(&mut w).add(Countdown {
            left: 2,
            ended: ended.clone(),
        });
        FxRuntime::update(&mut w, 0.1);
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 1);
        FxRuntime::update(&mut w, 0.1);
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 0);
        assert_eq!(ended.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn le_minuteur_enchaine_et_garde_les_taches_planifiees_en_route() {
        let mut w = world();
        let hits = Arc::new(AtomicU32::new(0));
        let h = hits.clone();
        Fx::new(&mut w).after(0.5, move |fx| {
            h.fetch_add(1, Ordering::Relaxed);
            let h2 = h.clone();
            // planifiée pendant la mise à jour : doit partir 0,5 s plus tard, pas tout de suite
            fx.after(0.5, move |_| {
                h2.fetch_add(10, Ordering::Relaxed);
            });
        });
        FxRuntime::update(&mut w, 0.3);
        assert_eq!(hits.load(Ordering::Relaxed), 0);
        FxRuntime::update(&mut w, 0.3);
        assert_eq!(hits.load(Ordering::Relaxed), 1);
        FxRuntime::update(&mut w, 0.3);
        assert_eq!(hits.load(Ordering::Relaxed), 1);
        FxRuntime::update(&mut w, 0.3);
        assert_eq!(hits.load(Ordering::Relaxed), 11);
        assert!((w.resource::<FxRuntime>().time - 1.2).abs() < 1e-5);
    }

    #[test]
    fn la_remise_a_zero_termine_tout() {
        let mut w = world();
        let ended = Arc::new(AtomicU32::new(0));
        let mut fx = Fx::new(&mut w);
        fx.add(Countdown {
            left: 100,
            ended: ended.clone(),
        });
        fx.after(1.0, |_| panic!("tâche d'une partie précédente"));
        fx.set_aura("a", |_| {});
        FxRuntime::reset(&mut w);
        assert_eq!(
            ended.load(Ordering::Relaxed),
            1,
            "`end` appelé : pools rendus"
        );
        let rt = w.resource::<FxRuntime>();
        assert_eq!((rt.active_effects(), rt.pending_tasks()), (0, 0));
        FxRuntime::update(&mut w, 2.0);
    }

    #[test]
    fn une_aura_remplacee_ou_terminee_est_annulee_une_fois() {
        let mut w = world();
        let n = Arc::new(AtomicU32::new(0));
        let mut fx = Fx::new(&mut w);
        let a = n.clone();
        let first = fx.set_aura("bouclier", move |_| {
            a.fetch_add(1, Ordering::Relaxed);
        });
        let b = n.clone();
        let second = fx.set_aura("bouclier", move |_| {
            b.fetch_add(100, Ordering::Relaxed);
        });
        assert_eq!(n.load(Ordering::Relaxed), 1, "l'ancienne est annulée");
        fx.forget_aura("bouclier", first);
        fx.end_aura("bouclier");
        assert_eq!(n.load(Ordering::Relaxed), 101);
        fx.forget_aura("bouclier", second);
        fx.end_aura("bouclier");
        assert_eq!(n.load(Ordering::Relaxed), 101);
    }

    #[test]
    fn teinte_ennemie_des_zones_au_sol() {
        let mut rt = FxRuntime::default();
        let c = Vec3::new(0.2, 0.6, 1.0);
        assert_eq!(rt.ground_color(c), c);
        rt.enemy_context = true;
        assert!((rt.ground_color(c) - c.lerp(ENEMY_TINT, 0.45)).length() < 1e-6);
        assert_eq!(rt.colors("ilyra_q"), rt.colors("ilyra_q"));
    }

    #[test]
    fn les_pools_recyclent_decals_rubans_et_sprites() {
        let mut w = world();
        w.init_resource::<Assets<Mesh>>();
        w.init_resource::<Assets<Image>>();
        w.init_resource::<Assets<DecalMaterial>>();
        w.init_resource::<Assets<RibbonMaterial>>();
        w.init_resource::<Assets<StandardMaterial>>();
        w.init_resource::<DecalMesh>();
        w.init_resource::<SpriteAssets>();
        let mut fx = Fx::new(&mut w);
        let d = fx.acquire_decal();
        let r = fx.acquire_ribbon(Vec3::X);
        let sp = fx.acquire_sprite(Vec3::ONE, 2.0);
        let (de, re, se) = (d.entity, r.entity, sp.entity);
        fx.release_decal(d);
        fx.release_ribbon(r);
        fx.release_sprite(sp);
        assert_eq!(fx.world().get::<Visibility>(de), Some(&Visibility::Hidden));
        // le pool rend les mêmes entités, visibles et remises à zéro
        let d = fx.acquire_decal();
        let r = fx.acquire_ribbon(Vec3::Y);
        let sp = fx.acquire_sprite(Vec3::ONE, 3.0);
        assert_eq!((d.entity, r.entity, sp.entity), (de, re, se));
        assert_eq!(
            fx.world().get::<Visibility>(se),
            Some(&Visibility::Inherited)
        );
        assert_eq!(
            fx.world().get::<Transform>(se).map(|t| t.scale.x),
            Some(3.0)
        );
        let ribbon = fx
            .world()
            .get::<Ribbon>(re)
            .map(|r| (r.width, r.life, r.alive()));
        assert_eq!(ribbon, Some((0.3, 0.35, 0)));
        let meshes = fx.world().query::<&Mesh3d>().iter(fx.world()).count();
        assert_eq!(meshes, 3, "aucune entité recréée");
    }

    #[test]
    fn coordonnees_de_jeu_vers_monde() {
        let p = Fx::world_pos(GameVec2 { x: 500.0, y: 200.0 }, 1.0);
        // `h` se compte au-dessus du sol, qui suit le relief du terrain (#274)
        let ground = crate::render::ground_height(500.0, 200.0);
        assert!((p - Vec3::new(5.0, ground + 1.0, -2.0)).length() < 1e-6);
        // au milieu de la rivière, le sol est creusé
        let river = Fx::world_pos(
            GameVec2 {
                x: 6000.0,
                y: 6000.0,
            },
            0.0,
        );
        assert!(river.y < -0.25, "{river}");
        assert!((Fx::scale(250.0) - 2.5).abs() < 1e-6);
    }
}
