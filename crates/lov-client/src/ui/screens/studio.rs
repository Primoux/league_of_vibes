//! Studio photo des champions : portage de `packages/client/src/ui/stage/studio.ts`.
//!
//! Rend le modèle 3D d'un champion dans une image (buste ou en pied), avec l'éclairage de portrait de la
//! scène (clé chaude, contre-jour à la couleur du champion), et la garde en cache. Les portraits s'en
//! servent pour les champions sans illustration peinte. Une photo à la fois, comme la file du web : la
//! scène vit quelques images (davantage pour la première, le temps que les shaders se compilent), puis
//! disparaît ; l'image reste.

use std::collections::{HashMap, VecDeque};

use bevy::app::Propagate;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use lov_shared::data::champions::get_champion;

use super::stage::{idle_snap, spawn_champion, studio_ambient, studio_lights};
use crate::fx::post::PostFxCamera;
use crate::render::champions::ChampionAssets;
use crate::ui::theme::css;

/// Calque des photos (les scènes vivantes prennent les suivants).
const LAYER: usize = 7;
/// Images de vie d'une photo, et de la première (compilation des shaders).
const FRAMES: u32 = 6;
const FIRST_FRAMES: u32 = 90;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShotKind {
    /// Tête et épaules, 256 × 256.
    Bust,
    /// En pied, 300 × 400.
    Full,
}

impl ShotKind {
    pub fn size(self) -> UVec2 {
        match self {
            Self::Bust => UVec2::new(256, 256),
            Self::Full => UVec2::new(300, 400),
        }
    }
}

/// Photos prêtes ou en attente, et photo en cours.
#[derive(Resource, Default)]
pub struct Studio {
    shots: HashMap<(String, ShotKind), Handle<Image>>,
    queue: VecDeque<(String, ShotKind, Handle<Image>)>,
    current: Option<(Entity, u32)>,
    taken: u32,
}

impl Studio {
    /// Image du champion (`getShot`) : elle se remplit quand la photo est prise. `None` si le champion
    /// est inconnu.
    pub fn shot(
        &mut self,
        images: &mut Assets<Image>,
        id: &str,
        kind: ShotKind,
    ) -> Option<Handle<Image>> {
        get_champion(id)?;
        let key = (id.to_string(), kind);
        if let Some(h) = self.shots.get(&key) {
            return Some(h.clone());
        }
        let size = kind.size();
        let h = images.add(Image::new_target_texture(
            size.x,
            size.y,
            TextureFormat::Bgra8UnormSrgb,
            None,
        ));
        self.shots.insert(key, h.clone());
        self.queue.push_back((id.to_string(), kind, h.clone()));
        Some(h)
    }
}

/// Cadre la caméra sur le modèle (`frame`) : `Bust` = tête et épaules, `Full` = en pied. `h` : hauteur du
/// modèle, posé au sol.
pub fn frame(kind: ShotKind, h: f32) -> Transform {
    let h = h.max(0.2);
    let (fov, aspect) = fov_aspect(kind);
    let center_y = h / 2.0;
    if kind == ShotKind::Bust {
        let focus = h - h * 0.3;
        let dist = h * 0.62 / 2.0 / (fov / 2.0).tan();
        Transform::from_xyz(dist * 0.42, focus + h * 0.06, dist * 0.9)
            .looking_at(Vec3::new(0.0, focus, 0.0), Vec3::Y)
    } else {
        // largeur du modèle ≈ 0,55 h : la hauteur l'emporte toujours en 3:4
        let visible = (h * 1.12).max(h * 0.55 * 1.15 / aspect);
        let dist = visible / 2.0 / (fov / 2.0).tan();
        Transform::from_xyz(dist * 0.3, center_y + h * 0.12, dist * 0.95)
            .looking_at(Vec3::new(0.0, center_y + h * 0.02, 0.0), Vec3::Y)
    }
}

fn fov_aspect(kind: ShotKind) -> (f32, f32) {
    let s = kind.size();
    let fov = if kind == ShotKind::Bust { 26f32 } else { 30.0 };
    (fov.to_radians(), s.x as f32 / s.y as f32)
}

/// Prend les photos une par une (`pump`).
fn pump(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut champs: ChampionAssets,
) {
    if let Some((scene, left)) = studio.current {
        if left > 0 {
            studio.current = Some((scene, left - 1));
            return;
        }
        commands.entity(scene).despawn();
        studio.current = None;
    }
    let Some((id, kind, image)) = studio.queue.pop_front() else {
        return;
    };
    let Some(def) = get_champion(&id) else {
        return;
    };
    let Some((model, view, h)) = spawn_champion(&mut commands, &mut meshes, &mut champs, &id)
    else {
        return;
    };
    // quelques pas d'animation pour sortir de la pose neutre
    if let Some(mut view) = view {
        let snap = idle_snap(&id);
        let mut last = None;
        for _ in 0..12 {
            last = Some(view.step(&snap, "idle", 1.0 / 30.0, Vec3::ZERO, 0.0));
        }
        if let Some(f) = last {
            for (j, t) in view.model.skeleton.joints.iter().zip(f.joints) {
                commands.entity(*j).insert(t);
            }
        }
    }
    let yaw = if kind == ShotKind::Bust { 0.35 } else { 0.25 };
    commands
        .entity(model)
        .insert(Transform::from_rotation(Quat::from_rotation_y(yaw)));
    let (fov, _) = fov_aspect(kind);
    let scene = commands
        .spawn((
            Transform::default(),
            Visibility::default(),
            Propagate(RenderLayers::layer(LAYER)),
        ))
        .with_children(|s| {
            s.spawn((
                Camera3d::default(),
                Camera {
                    order: -100,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                RenderTarget::Image(image.into()),
                Projection::Perspective(PerspectiveProjection {
                    fov,
                    near: 0.01,
                    far: 100.0,
                    ..default()
                }),
                PostFxCamera,
                studio_ambient(),
                frame(kind, h),
            ));
            studio_lights(s, css(&def.palette.primary));
        })
        .id();
    commands.entity(model).insert(ChildOf(scene));
    let frames = if studio.taken == 0 {
        FIRST_FRAMES
    } else {
        FRAMES
    };
    studio.taken += 1;
    studio.current = Some((scene, frames));
}

pub struct StudioPlugin;

impl Plugin for StudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Studio>().add_systems(Update, pump);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photo_en_cache_et_en_file_une_seule_fois() {
        let mut images = Assets::<Image>::default();
        let mut s = Studio::default();
        let a = s.shot(&mut images, "ilyra", ShotKind::Bust);
        let b = s.shot(&mut images, "ilyra", ShotKind::Bust);
        assert!(a.is_some() && a == b);
        assert!(s.shot(&mut images, "ilyra", ShotKind::Full).is_some());
        assert!(s.shot(&mut images, "inconnu", ShotKind::Bust).is_none());
        assert_eq!(s.queue.len(), 2);
    }

    #[test]
    fn cadrage_au_dessus_du_sol_et_vers_le_modele() {
        for kind in [ShotKind::Bust, ShotKind::Full] {
            let t = frame(kind, 1.8);
            assert!(t.translation.y > 0.0 && t.translation.z > 0.0);
            // la caméra regarde vers l'origine (−z)
            assert!(t.forward().z < 0.0);
        }
        // le buste est cadré plus près que le plan en pied
        assert!(
            frame(ShotKind::Bust, 1.8).translation.z < frame(ShotKind::Full, 1.8).translation.z
        );
    }
}
