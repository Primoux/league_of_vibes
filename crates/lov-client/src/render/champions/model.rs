//! Définition d'un modèle de champion et apparition dans le monde.
//!
//! Portage de la partie « modèle » de `packages/client/src/render/champions/ChampionView.ts`
//! (`ChampionModelDef`, `ModelColors`, `glowColor`, construction du modèle). L'animation et la vue
//! arrivent dans la PR suivante de #110.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use lov_shared::data::champions::get_champion;
use lov_shared::schema::ChampionClass;
use lov_shared::types::TeamOrNeutral;

use super::anim::pose::Clip;
use super::builder::{BuiltModel, JointMeshes, ModelBuilder, css};
use super::outline::OutlineMaterial;
use super::rig::{RigSpec, Skeleton};

/// Couleur d'équipe des champions (`TEAM_HEX`).
pub fn team_hex(team: TeamOrNeutral) -> &'static str {
    match team {
        TeamOrNeutral::Blue => "#3a7bff",
        TeamOrNeutral::Red => "#ff4a4a",
        TeamOrNeutral::Neutral => "#c9a227",
    }
}

/// Couleurs du champion, linéaires.
#[derive(Clone, Copy, Debug)]
pub struct ModelColors {
    pub primary: LinearRgba,
    pub secondary: LinearRgba,
    pub accent: LinearRgba,
    pub team: LinearRgba,
    /// Accent surexposé pour les parties lumineuses (bloom).
    pub glow: LinearRgba,
    pub class: ChampionClass,
}

impl ModelColors {
    /// Couleurs d'un champion connu, ou `None`.
    pub fn of(champion: &str, team: TeamOrNeutral) -> Option<Self> {
        let def = get_champion(champion)?;
        let p = &def.palette;
        Some(Self {
            primary: css(&p.primary),
            secondary: css(&p.secondary),
            accent: css(&p.accent),
            team: css(team_hex(team)),
            glow: glow_color(css(&p.accent)),
            class: def.class,
        })
    }
}

/// Accent surexposé, d'autant moins que l'accent est déjà clair (un blanc × 2 sature le bloom).
pub fn glow_color(c: LinearRgba) -> LinearRgba {
    let lum = 0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue;
    let k = 1.3 + (1.0 - lum.min(1.0)) * 1.4;
    LinearRgba::rgb(c.red * k, c.green * k, c.blue * k)
}

/// Construction d'un modèle : pièces ajoutées au builder, selon les couleurs et le squelette.
pub type ModelBuild = fn(&mut ModelBuilder, &ModelColors, &RigSpec);

/// Modèle d'un champion : squelette, clips propres, construction.
#[derive(Clone, Copy, Debug)]
pub struct ChampionModelDef {
    pub rig: RigSpec,
    /// Bras qui frappe (1 = droit, −1 = gauche) ; défaut 1.
    pub hand: Option<f32>,
    /// Amplitude générale des clips ; défaut 1.
    pub energy: Option<f32>,
    /// Clips qui remplacent les génériques.
    pub clips: &'static [(&'static str, Clip)],
    /// Échelle globale du modèle.
    pub scale: f32,
    /// Construction des pièces ; reçoit le squelette du modèle (`this.rig` du client web).
    pub build: ModelBuild,
    /// Port d'arme : angle (rad, autour de z, dans le repère du corps) que garde l'accessoire tenu pendant
    /// la locomotion. 0 = vertical (bâton), négatif = incliné vers l'avant. `None` = suit le bras.
    pub carry: Option<f32>,
    /// Angle de l'accessoire au sol à la mort (0 pour une lame le long de +x, −π/2 pour un bâton le long de
    /// +y).
    pub drop_angle: Option<f32>,
}

/// Matériaux d'un champion : un jeu par instance (la dissolution et l'invisibilité lui sont propres).
#[derive(Clone, Debug)]
pub struct ChampionMaterials {
    pub body: Handle<StandardMaterial>,
    pub metal: Handle<StandardMaterial>,
    pub glow: Handle<StandardMaterial>,
}

/// Modèle apparu : squelette et maillages par lot (pour l'invisibilité, la mort, les extras).
#[derive(Clone, Debug)]
pub struct SpawnedModel {
    pub skeleton: Skeleton,
    pub materials: ChampionMaterials,
    pub meshes: Vec<Entity>,
    pub outlines: Vec<Entity>,
    pub glows: Vec<Entity>,
}

/// Construit le modèle de `def` aux couleurs `colors`.
pub fn build_model(def: &ChampionModelDef, colors: &ModelColors) -> BuiltModel {
    let mut b = ModelBuilder::new();
    (def.build)(&mut b, colors, &def.rig);
    b.build(&def.rig, true)
}

/// Fait apparaître le squelette et les maillages d'un modèle construit.
pub fn spawn_model(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    outline: &Handle<OutlineMaterial>,
    def: &ChampionModelDef,
    built: BuiltModel,
) -> SpawnedModel {
    let skeleton = Skeleton::spawn(commands, &def.rig);
    commands
        .entity(skeleton.root)
        .insert(Transform::from_scale(Vec3::splat(def.scale)));
    let lit = |rough: f32, metal: f32| StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: rough,
        metallic: metal,
        ..default()
    };
    let mats = ChampionMaterials {
        body: materials.add(if built.sculpted {
            lit(0.7, 0.0)
        } else {
            lit(0.78, 0.05)
        }),
        metal: materials.add(lit(0.32, 0.72)),
        // `MeshBasicMaterial({ vertexColors, toneMapped: false })` : couleurs HDR non éclairées
        glow: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            ..default()
        }),
    };
    let mut out = SpawnedModel {
        skeleton,
        materials: mats.clone(),
        meshes: Vec::new(),
        outlines: Vec::new(),
        glows: Vec::new(),
    };
    let joints = out.skeleton.joints;
    let mut lot = |commands: &mut Commands, lot: JointMeshes, kind: Lot| -> Vec<Entity> {
        let mut list = Vec::new();
        for (j, mesh) in lot.into_iter().enumerate() {
            let Some(mesh) = mesh else {
                continue;
            };
            let mut e = commands.spawn((
                Mesh3d(meshes.add(mesh)),
                Transform::default(),
                ChildOf(joints[j]),
            ));
            match kind {
                Lot::Solid(m) => e.insert(MeshMaterial3d(m.clone())),
                Lot::Glow(m) => e.insert((MeshMaterial3d(m.clone()), NotShadowCaster)),
                Lot::Outline => e.insert((MeshMaterial3d(outline.clone()), NotShadowCaster)),
            };
            list.push(e.id());
        }
        list
    };
    out.meshes = lot(commands, built.body, Lot::Solid(&mats.body));
    out.meshes
        .extend(lot(commands, built.metal, Lot::Solid(&mats.metal)));
    out.outlines = lot(commands, built.outline_body, Lot::Outline);
    out.outlines
        .extend(lot(commands, built.outline_metal, Lot::Outline));
    out.glows = lot(commands, built.glow, Lot::Glow(&mats.glow));
    out
}

/// Lot de maillages et son matériau.
#[derive(Clone, Copy)]
enum Lot<'a> {
    Solid(&'a Handle<StandardMaterial>),
    Glow(&'a Handle<StandardMaterial>),
    Outline,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_surexpose_selon_sa_clarte() {
        let dark = glow_color(LinearRgba::rgb(0.1, 0.0, 0.0));
        assert!((dark.red - 0.1 * (1.3 + (1.0 - 0.02126) * 1.4)).abs() < 1e-5);
        let white = glow_color(LinearRgba::WHITE);
        assert!(
            (white.red - 1.3).abs() < 1e-5,
            "un blanc n'est surexposé que de 1,3"
        );
    }

    #[test]
    fn couleurs_d_un_champion_connu() {
        let c = ModelColors::of("kaelen", TeamOrNeutral::Red);
        assert!(c.is_some());
        assert_eq!(c.map(|c| c.team), Some(css("#ff4a4a")));
        assert!(ModelColors::of("inconnu", TeamOrNeutral::Red).is_none());
    }
}
