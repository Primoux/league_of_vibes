//! Rendu de la carte « Faille des Vibes » : portage de `packages/client/src/render/map/` (issue #108).
//!
//! - `geometry` : relief du terrain (`terrain_height`), distances aux voies et à la rivière ;
//! - `noise` : texture de bruit tuilable partagée par les shaders du décor ;
//! - `masks` + `terrain` : sol avec relief, peint par shader (herbe, voies, sable, dallage, caustiques) ;
//! - `fog` : brouillard de guerre (texture de visibilité lue par le sol, la végétation, l'eau et la minicarte) ;
//! - `water` : rivière animée (vagues, reflets, écume des berges) ;
//! - `cliffs` + `vegetation` + `vgeo` : murs en falaises, arbres, rochers, brousses animées par le vent,
//!   touffes et fleurs (placement identique au client web, `rng`).
//!
//! Murs et brousses viennent de `lov_shared::map` (`WALLS`, `BRUSHES`).

use bevy::prelude::*;
use lov_shared::constants;

mod cliffs;
pub mod fog;
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
        app.init_resource::<fog::FogOfWar>()
            .add_plugins((
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
                    ),
                )
                    .chain(),
            )
            .add_systems(Update, (vegetation::fade_brushes, fog::update_fog));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::from_shared;
    use lov_shared::map::{BRUSHES, WALLS};

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
