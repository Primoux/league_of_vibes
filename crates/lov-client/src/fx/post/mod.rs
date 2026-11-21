//! Post-traitement : cible HDR + MSAA, bloom sélectif par luminance, tone mapping ACES, étalonnage de sortie.
//!
//! Portage de `packages/client/src/fx/post/PostFx.ts`, en s'appuyant sur Bevy là où il fait déjà le travail :
//! - **HDR + MSAA ×4** : `Hdr` sur la caméra (MSAA ×4 est le défaut de Bevy) ;
//! - **filet anti-NaN** : passe plein écran `Sanitize` juste avant le bloom (NaN/Inf → noir, HDR plafonné à 64),
//!   sans quoi le flou étalerait un pixel invalide en blocs noirs ;
//! - **bloom** : `Bloom` de Bevy en mode additif, seuil 0,92 (seuls les FX en couleurs HDR brillent, la scène
//!   éclairée normalement reste en dessous). Le flou de Bevy n'est pas celui d'`UnrealBloomPass` : la force est
//!   réglée à l'œil pour un rendu proche ;
//! - **tone mapping** : `Tonemapping::AcesFitted`, la même approximation (Hill) que `ACESFilmicToneMapping` de
//!   three.js, avec son exposition effective (`toneMappingExposure` 1,05 ÷ 0,6) en `ColorGrading` ;
//! - **étalonnage** : passe plein écran `Grade` après le tone mapping, mêmes formules que le client web
//!   (saturation, teinte des ombres, gain des hautes lumières, contraste, vignette) ; filet anti-NaN ;
//! - **qualité basse** : ni HDR, ni bloom, ni étalonnage : rendu direct, tone mapping conservé, comme le
//!   client web.
//!
//! La caméra appartient à l'area `render`, qui l'inscrit au post-traitement avec le marqueur `PostFxCamera`
//! (les autres caméras, minimap ou portraits, n'y passent pas). À son arrivée, `apply_post` règle le tone
//! mapping et l'exposition (une seule fois : la caméra peut ensuite les changer) ; quand `PostFxSettings`
//! change, il ne touche qu'au HDR, au bloom et aux passes plein écran. La distorsion de chaleur arrive dans
//! une PR suivante (elle demande un second rendu de la scène).

use bevy::camera::Hdr;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::core_pipeline::schedule::Core3dSystems;
use bevy::core_pipeline::tonemapping::{Tonemapping, tonemapping};
use bevy::ecs::schedule::ScheduleConfigs;
use bevy::ecs::system::BoxedSystem;
use bevy::post_process::bloom::{Bloom, BloomPrefilter, bloom};
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use bevy::render::view::ColorGrading;
use bevy::shader::ShaderRef;

const GRADE_SHADER: &str = "embedded://lov_client/fx/post/grade.wesl";
const SANITIZE_SHADER: &str = "embedded://lov_client/fx/post/sanitize.wesl";

/// `toneMappingExposure` du client web (1,05) et le facteur interne (÷ 0,6) de `ACESFilmicToneMapping`.
const EXPOSURE: f32 = 1.05 / 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FxQuality {
    #[default]
    High,
    Low,
}

/// Réglages d'étalonnage (appliqués après le tone mapping, avant la conversion sRGB). C'est aussi la
/// passe plein écran : ce composant sur la caméra l'active.
#[derive(Component, ExtractComponent, Clone, Copy, Debug, PartialEq, ShaderType)]
#[extract_app(RenderApp)]
pub struct Grade {
    /// Teinte ajoutée aux ombres (linéaire, petites valeurs).
    pub lift: Vec3,
    /// Saturation (1 = neutre).
    pub saturation: f32,
    /// Multiplicateur des hautes lumières.
    pub gain: Vec3,
    /// Contraste autour du gris moyen (1 = neutre).
    pub contrast: f32,
    /// Assombrissement des coins (0 = aucun).
    pub vignette: f32,
}

impl Grade {
    /// `DEFAULT_GRADE` du client web : ombres froides, hautes lumières chaudes, légère vignette.
    pub const DEFAULT: Self = Self {
        lift: Vec3::new(0.004, 0.008, 0.02),
        saturation: 1.07,
        gain: Vec3::new(1.03, 1.0, 0.95),
        contrast: 1.06,
        vignette: 0.28,
    };
}

impl Default for Grade {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl FullscreenMaterial for Grade {
    fn fragment_shader() -> ShaderRef {
        GRADE_SHADER.into()
    }

    /// Après le tone mapping (le défaut de Bevy est avant) : l'étalonnage du client web travaille sur
    /// l'image déjà ramenée dans [0, 1].
    fn schedule_configs(system: ScheduleConfigs<BoxedSystem>) -> ScheduleConfigs<BoxedSystem> {
        system.in_set(Core3dSystems::PostProcess).after(tonemapping)
    }
}

/// Marqueur posé par l'area `render` sur la caméra de jeu : seule cette caméra reçoit le post-traitement.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PostFxCamera;

/// Filet anti-NaN/Inf avant le bloom (passe plein écran, active avec le HDR).
#[derive(Component, ExtractComponent, Clone, Copy, Debug, PartialEq, ShaderType)]
#[extract_app(RenderApp)]
pub struct Sanitize {
    /// Plafond du HDR, comme `min(c, 64)` du client web.
    pub max_hdr: f32,
}

impl Default for Sanitize {
    fn default() -> Self {
        Self { max_hdr: 64.0 }
    }
}

impl FullscreenMaterial for Sanitize {
    fn fragment_shader() -> ShaderRef {
        SANITIZE_SHADER.into()
    }

    /// Avant le bloom, qui lit l'image HDR et en étalerait les NaN.
    fn schedule_configs(system: ScheduleConfigs<BoxedSystem>) -> ScheduleConfigs<BoxedSystem> {
        system
            .in_set(Core3dSystems::PostProcess)
            .before(bloom)
            .before(tonemapping)
    }
}

/// Réglages du post-traitement des caméras `PostFxCamera`.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PostFxSettings {
    pub quality: FxQuality,
    /// Force du bloom (0 = coupé : ses passes sont sautées).
    pub bloom: f32,
    /// Seuil de luminance du bloom.
    pub threshold: f32,
    pub grade: Grade,
}

impl Default for PostFxSettings {
    fn default() -> Self {
        Self {
            quality: FxQuality::High,
            bloom: 0.15,
            threshold: 0.92,
            grade: Grade::DEFAULT,
        }
    }
}

impl PostFxSettings {
    /// HDR (et donc filet anti-NaN) seulement en qualité haute : la basse rend directement à l'écran.
    pub fn hdr(&self) -> bool {
        self.quality == FxQuality::High
    }

    /// Bloom de Bevy correspondant aux réglages, `None` s'il est coupé (qualité basse ou force nulle).
    pub fn bloom(&self) -> Option<Bloom> {
        (self.quality == FxQuality::High && self.bloom > 0.0).then_some(Bloom {
            intensity: self.bloom,
            prefilter: BloomPrefilter {
                threshold: self.threshold,
                threshold_softness: 0.1,
            },
            ..Bloom::OLD_SCHOOL
        })
    }

    /// Étalonnage à appliquer, `None` en qualité basse.
    pub fn grade(&self) -> Option<Grade> {
        (self.quality == FxQuality::High).then_some(self.grade)
    }
}

pub struct PostFxPlugin;

impl Plugin for PostFxPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "grade.wesl");
        bevy::asset::embedded_asset!(app, "sanitize.wesl");
        app.add_plugins((
            FullscreenMaterialPlugin::<Sanitize>::default(),
            FullscreenMaterialPlugin::<Grade>::default(),
        ))
        .init_resource::<PostFxSettings>()
        .add_systems(PostUpdate, apply_post);
    }
}

/// Règle tone mapping et exposition des caméras qui arrivent (une fois), puis pose ou retire HDR, filet,
/// bloom et étalonnage selon la qualité : sur les nouvelles caméras, ou sur toutes quand les réglages changent.
fn apply_post(
    mut commands: Commands,
    settings: Res<PostFxSettings>,
    mut added: Query<(Entity, &mut ColorGrading), Added<PostFxCamera>>,
    all: Query<Entity, With<PostFxCamera>>,
) {
    for (entity, mut grading) in &mut added {
        grading.global.exposure = EXPOSURE.log2();
        commands.entity(entity).insert(Tonemapping::AcesFitted);
        if !settings.is_changed() {
            apply_quality(&mut commands, entity, &settings);
        }
    }
    if settings.is_changed() {
        for entity in &all {
            apply_quality(&mut commands, entity, &settings);
        }
    }
}

fn apply_quality(commands: &mut Commands, entity: Entity, settings: &PostFxSettings) {
    let mut e = commands.entity(entity);
    if settings.hdr() {
        e.insert_if_new((Hdr, Sanitize::default()));
    } else {
        e.remove::<(Hdr, Sanitize)>();
    }
    match settings.bloom() {
        Some(bloom) => e.insert(bloom),
        None => e.remove::<Bloom>(),
    };
    match settings.grade() {
        Some(grade) => e.insert(grade),
        None => e.remove::<Grade>(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualite_basse_coupe_bloom_et_etalonnage() {
        let mut s = PostFxSettings::default();
        let bloom = s.bloom().expect("bloom en qualité haute");
        assert_eq!(bloom.prefilter.threshold, 0.92);
        assert_eq!(bloom.intensity, 0.15);
        assert_eq!(s.grade(), Some(Grade::DEFAULT));
        assert!(s.hdr());
        s.bloom = 0.0;
        assert!(s.bloom().is_none(), "force nulle : passes sautées");
        s.bloom = 0.15;
        s.quality = FxQuality::Low;
        assert!(s.bloom().is_none() && s.grade().is_none() && !s.hdr());
    }

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<PostFxSettings>()
            .add_systems(Update, apply_post);
        app
    }

    #[test]
    fn camera_marquee_recoit_le_post_traitement() {
        let mut app = app();
        let cam = app
            .world_mut()
            .spawn((PostFxCamera, Tonemapping::None, ColorGrading::default()))
            .id();
        let autre = app
            .world_mut()
            .spawn((Tonemapping::None, ColorGrading::default()))
            .id();
        app.update();

        let w = app.world();
        // 2^EV = 1,75 : l'exposition effective d'ACESFilmicToneMapping avec toneMappingExposure 1,05
        let ev = w.get::<ColorGrading>(cam).unwrap().global.exposure;
        assert!((2f32.powf(ev) - 1.75).abs() < 1e-5);
        assert_eq!(w.get::<Tonemapping>(cam), Some(&Tonemapping::AcesFitted));
        assert!(w.get::<Hdr>(cam).is_some() && w.get::<Sanitize>(cam).is_some());
        assert!(w.get::<Bloom>(cam).is_some() && w.get::<Grade>(cam).is_some());

        assert_eq!(w.get::<Tonemapping>(autre), Some(&Tonemapping::None));
        assert_eq!(w.get::<ColorGrading>(autre).unwrap().global.exposure, 0.0);
        assert!(w.get::<Hdr>(autre).is_none() && w.get::<Bloom>(autre).is_none());
    }

    #[test]
    fn qualite_basse_rend_sans_hdr_et_garde_les_reglages_de_la_camera() {
        let mut app = app();
        let cam = app
            .world_mut()
            .spawn((PostFxCamera, Tonemapping::None, ColorGrading::default()))
            .id();
        app.update();

        // La caméra (area render) change ses réglages après coup : un changement de qualité les garde.
        let mut grading = app.world_mut().get_mut::<ColorGrading>(cam).unwrap();
        grading.global.post_saturation = 0.5;
        app.world_mut()
            .entity_mut(cam)
            .insert(Tonemapping::TonyMcMapface);
        app.world_mut().resource_mut::<PostFxSettings>().quality = FxQuality::Low;
        app.update();

        let w = app.world();
        assert!(w.get::<Hdr>(cam).is_none() && w.get::<Sanitize>(cam).is_none());
        assert!(w.get::<Bloom>(cam).is_none() && w.get::<Grade>(cam).is_none());
        assert_eq!(w.get::<Tonemapping>(cam), Some(&Tonemapping::TonyMcMapface));
        assert_eq!(
            w.get::<ColorGrading>(cam).unwrap().global.post_saturation,
            0.5
        );

        app.world_mut().resource_mut::<PostFxSettings>().quality = FxQuality::High;
        app.update();
        let w = app.world();
        assert!(w.get::<Hdr>(cam).is_some() && w.get::<Sanitize>(cam).is_some());
        assert!(w.get::<Bloom>(cam).is_some() && w.get::<Grade>(cam).is_some());
    }
}
