//! Post-traitement : cible HDR + MSAA, bloom sélectif par luminance, tone mapping ACES, étalonnage de sortie.
//!
//! Portage de `packages/client/src/fx/post/PostFx.ts`, en s'appuyant sur Bevy là où il fait déjà le travail :
//! - **HDR + MSAA ×4** : `Hdr` sur la caméra (MSAA ×4 est le défaut de Bevy) ;
//! - **bloom** : `Bloom` de Bevy en mode additif, seuil 0,92 (seuls les FX en couleurs HDR brillent, la scène
//!   éclairée normalement reste en dessous). Le flou de Bevy n'est pas celui d'`UnrealBloomPass` : la force est
//!   réglée à l'œil pour un rendu proche ;
//! - **tone mapping** : `Tonemapping::AcesFitted`, la même approximation (Hill) que `ACESFilmicToneMapping` de
//!   three.js, avec son exposition effective (`toneMappingExposure` 1,05 ÷ 0,6) en `ColorGrading` ;
//! - **étalonnage** : passe plein écran `Grade` après le tone mapping, mêmes formules que le client web
//!   (saturation, teinte des ombres, gain des hautes lumières, contraste, vignette) ; filet anti-NaN ;
//! - **qualité basse** : ni bloom ni étalonnage (tone mapping conservé, comme le rendu direct du client web).
//!
//! La caméra appartient à l'area `render` : on n'y touche pas, `apply_post` ajoute ces composants à toute
//! `Camera3d` et les remet à jour quand `PostFxSettings` change. La distorsion de chaleur arrive dans une PR
//! suivante (elle demande un second rendu de la scène).

use bevy::camera::Hdr;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::core_pipeline::schedule::Core3dSystems;
use bevy::core_pipeline::tonemapping::{Tonemapping, tonemapping};
use bevy::ecs::schedule::ScheduleConfigs;
use bevy::ecs::system::BoxedSystem;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use bevy::render::view::{ColorGrading, ColorGradingGlobal};
use bevy::shader::ShaderRef;

const GRADE_SHADER: &str = "embedded://lov_client/fx/post/grade.wesl";

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

/// Réglages du post-traitement, appliqués à toutes les caméras 3D.
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
    /// Bloom de Bevy correspondant aux réglages, `None` s'il est coupé (qualité basse ou force nulle).
    pub fn bloom(&self) -> Option<Bloom> {
        (self.quality == FxQuality::High && self.bloom > 0.0).then_some(Bloom {
            intensity: self.bloom,
            prefilter: BloomPrefilter {
                threshold: self.threshold,
                threshold_softness: 0.1,
            },
            composite_mode: BloomCompositeMode::Additive,
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
        app.add_plugins(FullscreenMaterialPlugin::<Grade>::default())
            .init_resource::<PostFxSettings>()
            .add_systems(PostUpdate, apply_post);
    }
}

/// Pose (ou retire) les composants de post-traitement sur les caméras 3D nouvelles, ou sur toutes quand les
/// réglages changent.
fn apply_post(
    mut commands: Commands,
    settings: Res<PostFxSettings>,
    all: Query<Entity, With<Camera3d>>,
    added: Query<Entity, Added<Camera3d>>,
) {
    let cameras: Vec<Entity> = if settings.is_changed() {
        all.iter().collect()
    } else {
        added.iter().collect()
    };
    for entity in cameras {
        let mut e = commands.entity(entity);
        e.insert((
            Hdr,
            Tonemapping::AcesFitted,
            ColorGrading {
                global: ColorGradingGlobal {
                    exposure: EXPOSURE.log2(),
                    ..default()
                },
                ..default()
            },
        ));
        match settings.bloom() {
            Some(bloom) => e.insert(bloom),
            None => e.remove::<Bloom>(),
        };
        match settings.grade() {
            Some(grade) => e.insert(grade),
            None => e.remove::<Grade>(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualite_basse_coupe_bloom_et_etalonnage() {
        let mut s = PostFxSettings::default();
        let bloom = s.bloom().expect("bloom en qualité haute");
        assert_eq!(bloom.prefilter.threshold, 0.92);
        assert_eq!(bloom.composite_mode, BloomCompositeMode::Additive);
        assert_eq!(s.grade(), Some(Grade::DEFAULT));
        s.bloom = 0.0;
        assert!(s.bloom().is_none(), "force nulle : passes sautées");
        s.bloom = 0.15;
        s.quality = FxQuality::Low;
        assert!(s.bloom().is_none() && s.grade().is_none());
    }

    #[test]
    fn exposition_de_three() {
        // 2^EV = 1,75 : l'exposition effective d'ACESFilmicToneMapping avec toneMappingExposure 1,05
        assert!((2f32.powf(EXPOSURE.log2()) - 1.75).abs() < 1e-5);
    }
}
