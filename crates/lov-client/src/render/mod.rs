//! Rendu 3D : caméra, vues d'entités, carte.

use bevy::prelude::*;
use lov_shared::constants;
use lov_shared::types::Vec2 as SharedVec2;

pub mod camera;
// Une partie de l'API des champions (lot métal, surfaces sculptées, `slab`, hauteur du modèle…) ne sert
// qu'aux modèles dédiés et aux FX, portés dans les tranches de champions (#127–#153).
#[allow(dead_code)]
pub mod champions;
pub mod entities;
pub mod healthbars;
pub mod map;
pub mod models;
pub mod perf;
pub mod stress;
pub mod viewlab;
pub mod views;

/// 1 unité de jeu = 0,01 unité 3D (`RENDER_SCALE` de `lov-shared`).
pub const RENDER_SCALE: f32 = constants::RENDER_SCALE as f32;

/// Position `lov-shared` (f64) → vecteur de jeu Bevy (f32).
pub fn from_shared(p: SharedVec2) -> Vec2 {
    Vec2::new(p.x as f32, p.y as f32)
}

/// Vecteur de jeu Bevy (f32) → position `lov-shared` (f64).
pub fn to_shared(p: Vec2) -> SharedVec2 {
    SharedVec2 {
        x: f64::from(p.x),
        y: f64::from(p.y),
    }
}

/// Jeu `(x, y)` → monde `(x·S, h, −y·S)`, comme `toWorld` du client web.
pub fn to_world(p: Vec2, h: f32) -> Vec3 {
    Vec3::new(p.x * RENDER_SCALE, h, -p.y * RENDER_SCALE)
}

/// Hauteur monde du sol sous le point de jeu `(x, y)` : rivière creusée, léger relief de la jungle
/// (`terrainHeight(x, y) * S` du client web).
pub fn ground_height(x: f64, y: f64) -> f32 {
    (map::geometry::terrain_height(x, y) * f64::from(RENDER_SCALE)) as f32
}

/// Jeu → monde, posé sur le relief du terrain, `lift` au-dessus du sol.
pub fn to_ground(p: Vec2, lift: f32) -> Vec3 {
    to_world(p, ground_height(f64::from(p.x), f64::from(p.y)) + lift)
}

/// Monde → jeu, comme `toGame` du client web.
pub fn to_game(v: Vec3) -> Vec2 {
    Vec2::new(v.x / RENDER_SCALE, -v.z / RENDER_SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour_jeu_monde() {
        let p = Vec2::new(1500.0, 9000.0);
        assert_eq!(to_world(p, 0.0), Vec3::new(15.0, 0.0, -90.0));
        assert!((to_game(to_world(p, 2.0)) - p).length() < 0.01);
    }

    /// #274 : au milieu de la rivière, le sol est creusé ; sur une voie, il est plat.
    #[test]
    fn le_sol_suit_le_relief() {
        use map::geometry::RIVER_DEPTH;
        let river = to_ground(Vec2::new(6000.0, 6000.0), 0.0);
        assert!(
            river.y < -(RIVER_DEPTH as f32) * 0.9 * RENDER_SCALE,
            "{river}"
        );
        assert_eq!(to_game(river), Vec2::new(6000.0, 6000.0));
        let lane = to_ground(Vec2::new(1100.0, 6000.0), 0.5);
        assert!((lane.y - 0.5).abs() < 0.01, "{lane}");
    }
}
