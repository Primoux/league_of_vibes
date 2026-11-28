//! Rivière animée (shader d'eau stylisé) : portage de `packages/client/src/render/map/water.ts`.
//!
//! Bande le long de x + y = MAP_SIZE, courant vers le bas-droite. Écarts avec le client web : l'horloge est
//! `globals.time` de Bevy (`decorTime`), le brouillard de guerre assombrit l'eau hors de vue (`fog.rs`) ; et
//! la courbe de tons est appliquée par le post-traitement au lieu du shader.

use std::f32::consts::{FRAC_PI_4, SQRT_2};

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use lov_shared::constants::MAP_SIZE;
use lov_shared::map::RIVER_HALF_WIDTH;

use super::geometry::RIVER_DEPTH;
use super::noise::NoiseTexture;
use super::terrain::lin;
use crate::render::RENDER_SCALE;

const SHADER: &str = "embedded://lov_client/render/map/water.wesl";

/// Longueur de la rivière, de l'embouchure top à l'embouchure bot (unités de jeu).
pub const RIVER_LENGTH: f32 = 9600.0 * SQRT_2 * 0.94;
/// Largeur de la nappe d'eau (unités de jeu).
pub const RIVER_WIDTH: f32 = (RIVER_HALF_WIDTH as f32 + 60.0) * 2.0;

/// Uniforms de `water.wesl`.
#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct RiverUniform {
    size: Vec2,
    _pad: Vec2,
    deep: Vec4,
    shallow: Vec4,
    foam: Vec4,
    sky: Vec4,
    sun: Vec4,
    sun_dir: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct RiverMaterial {
    #[uniform(0)]
    u: RiverUniform,
    #[texture(1)]
    #[sampler(2)]
    noise: Handle<Image>,
    /// Visibilité du brouillard de guerre ([`super::fog::FogOfWar`]).
    #[texture(3)]
    #[sampler(4)]
    fog: Handle<Image>,
}

impl Material for RiverMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// `PlaneGeometry(len, width).rotateX(-π/2)` : axe x local le long de la rivière, UV de three.js
/// (u de gauche à droite, v = 1 sur le bord −z).
fn river_mesh(len: f32, width: f32) -> Mesh {
    let (hl, hw) = (len / 2.0, width / 2.0);
    let positions = vec![
        [-hl, 0.0, -hw],
        [hl, 0.0, -hw],
        [-hl, 0.0, hw],
        [hl, 0.0, hw],
    ];
    let uvs = vec![[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 4])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(vec![0, 2, 1, 2, 3, 1]))
}

/// Position et orientation de la nappe : au centre de la carte, sous le niveau du sol, axe x local suivant
/// la direction de jeu (1, −1), soit (1, 0, 1) en monde.
pub fn river_transform() -> Transform {
    let half = MAP_SIZE as f32 / 2.0 * RENDER_SCALE;
    Transform::from_xyz(half, -(RIVER_DEPTH as f32) * 0.45 * RENDER_SCALE, -half)
        .with_rotation(Quat::from_rotation_y(-FRAC_PI_4))
}

pub(super) fn spawn_river(
    mut commands: Commands,
    noise: Res<NoiseTexture>,
    fog: Res<super::fog::FogOfWar>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RiverMaterial>>,
) {
    let size = Vec2::new(RIVER_LENGTH, RIVER_WIDTH) * RENDER_SCALE;
    let material = materials.add(RiverMaterial {
        u: RiverUniform {
            size,
            _pad: Vec2::ZERO,
            deep: lin(0x0f4f72),
            shallow: lin(0x3fb0bf),
            foam: lin(0xeefbff),
            sky: lin(0xa8d8f0),
            sun: lin(0xfff0d0),
            sun_dir: Vec3::new(14.0, 30.0, 9.0).normalize().extend(0.0),
        },
        noise: noise.0.clone(),
        fog: fog.texture.clone(),
    });
    commands.spawn((
        Name::new("water"),
        Mesh3d(meshes.add(river_mesh(size.x, size.y))),
        MeshMaterial3d(material),
        river_transform(),
        NotShadowCaster,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::to_game;

    #[test]
    fn la_riviere_suit_l_axe_x_plus_y_egal_map_size() {
        let t = river_transform();
        let half_len = RIVER_LENGTH * RENDER_SCALE / 2.0;
        let size = MAP_SIZE as f32;
        for end in [-half_len, half_len] {
            let p = to_game(t.transform_point(Vec3::new(end, 0.0, 0.0)));
            assert!(
                (p.x + p.y - size).abs() < 0.5,
                "extrémité {p:?} hors de l'axe"
            );
            // embouchures dans les coins top et bot, à 94 % de la diagonale 1200 → 10800
            assert!((p.x - size / 2.0).abs() > 4000.0);
        }
        let bank =
            to_game(t.transform_point(Vec3::new(0.0, 0.0, RIVER_WIDTH * RENDER_SCALE / 2.0)));
        let across = (bank.x + bank.y - size).abs() / SQRT_2;
        assert!((across - RIVER_WIDTH / 2.0).abs() < 0.5);
        assert!(t.translation.y < 0.0);
    }
}
