//! Rendu de la carte « Faille des Vibes » : portage de `packages/client/src/render/map/` (issue #108).
//!
//! - `geometry` : relief du terrain (`terrain_height`), distances aux voies et à la rivière ;
//! - `noise` : texture de bruit tuilable partagée par les shaders du décor ;
//! - `masks` + `terrain` : sol avec relief, peint par shader (herbe, voies, sable, dallage, caustiques) ;
//! - `water` : rivière animée (vagues, reflets, écume des berges) ;
//! - `cliffs` + `vegetation` + `vgeo` : murs en falaises, arbres et rochers (placement identique au
//!   client web, `rng`).
//!
//! Murs et brousses viennent de `lov_shared::map` (`WALLS`, `BRUSHES`).

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use lov_shared::constants;
use lov_shared::map::BRUSHES;

use super::{from_shared, to_world};

mod cliffs;
pub mod geometry;
mod masks;
pub mod noise;
mod rng;
pub mod terrain;
pub mod vegetation;
mod vgeo;
pub mod water;

pub const MAP_SIZE: f32 = constants::MAP_SIZE as f32;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "terrain.wesl");
        bevy::asset::embedded_asset!(app, "water.wesl");
        bevy::asset::embedded_asset!(app, "cliff.wesl");
        bevy::asset::embedded_asset!(app, "foliage.wesl");
        app.add_plugins((
            MaterialPlugin::<terrain::TerrainMaterial>::default(),
            MaterialPlugin::<water::RiverMaterial>::default(),
            MaterialPlugin::<cliffs::CliffMaterial>::default(),
            MaterialPlugin::<vegetation::FoliageMaterial>::default(),
        ))
        .add_systems(
            Startup,
            (
                noise::create_noise_texture,
                (
                    terrain::spawn_terrain,
                    water::spawn_river,
                    vegetation::spawn_vegetation,
                    spawn_brush_tops,
                ),
            )
                .chain(),
        );
    }
}

/// Brousses en aplats verts (prototype), en attendant le portage des herbes hautes (`buildBrushes`).
fn spawn_brush_tops(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for p in BRUSHES.iter() {
        let base = positions.len() as u32;
        positions.extend(p.iter().map(|v| to_world(from_shared(*v), 0.03).to_array()));
        for i in 1..p.len() as u32 - 1 {
            indices.extend([base, base + i, base + i + 1]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices));
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.5, 0.2),
            perceptual_roughness: 0.95,
            ..default()
        })),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::map::WALLS;

    type Polygon = Vec<Vec2>;

    /// Polygones `lov-shared` (f64, anti-horaires) → polygones de jeu Bevy (f32).
    fn polygons(source: &[lov_shared::map::Polygon]) -> Vec<Polygon> {
        source
            .iter()
            .map(|p| p.iter().copied().map(from_shared).collect())
            .collect()
    }

    #[test]
    fn la_carte_a_28_murs_et_32_brousses_en_sens_anti_horaire() {
        let walls = polygons(&WALLS);
        let brushes = polygons(&BRUSHES);
        assert_eq!(walls.len(), 28);
        assert_eq!(brushes.len(), 32);
        for p in walls.iter().chain(&brushes) {
            let area: f32 = (0..p.len())
                .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
                .sum();
            assert!(area > 0.0);
        }
    }

    #[test]
    fn la_carte_est_symetrique_par_rotation_de_180_degres() {
        let walls = polygons(&WALLS);
        let center = |p: &Polygon| p.iter().copied().sum::<Vec2>() / p.len() as f32;
        for (blue, red) in walls[..14].iter().zip(&walls[14..]) {
            assert!((center(blue) + center(red) - Vec2::splat(MAP_SIZE)).length() < 0.01);
        }
    }
}
