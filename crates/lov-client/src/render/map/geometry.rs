//! Fonctions géométriques pures sur la carte : portage de `packages/client/src/render/map/geometry.ts`.
//!
//! Calculs en `f64` et en unités de jeu, comme le TypeScript.

use std::f64::consts::FRAC_1_SQRT_2;

use lov_shared::constants::MAP_SIZE;
use lov_shared::map::{LANE_HALF_WIDTH, LANE_PATHS, RIVER_HALF_WIDTH};
use lov_shared::types::Vec2;

/// Profondeur de la rivière (unités de jeu).
pub const RIVER_DEPTH: f64 = 32.0;

pub fn dist_to_segment(px: f64, py: f64, a: Vec2, b: Vec2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 {
        0.0
    } else {
        (((px - a.x) * dx + (py - a.y) * dy) / l2).clamp(0.0, 1.0)
    };
    (px - a.x - t * dx).hypot(py - a.y - t * dy)
}

/// Distance à la voie la plus proche (axe central).
pub fn dist_to_lane(x: f64, y: f64) -> f64 {
    let mut best = f64::INFINITY;
    for path in [&LANE_PATHS.top, &LANE_PATHS.mid, &LANE_PATHS.bot] {
        for w in path.windows(2) {
            best = best.min(dist_to_segment(x, y, w[0], w[1]));
        }
    }
    best
}

/// Distance (perpendiculaire) à l'axe de la rivière x + y = MAP_SIZE.
pub fn dist_to_river(x: f64, y: f64) -> f64 {
    (x + y - MAP_SIZE).abs() * FRAC_1_SQRT_2
}

/// `smoothstep` du TypeScript (bornes quelconques).
pub fn smooth(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Hauteur du terrain (unités de jeu) : la rivière est creusée en pente douce, léger relief dans la
/// jungle, voies plates. Les unités sont posées à cette hauteur.
pub fn terrain_height(x: f64, y: f64) -> f64 {
    let dr = dist_to_river(x, y);
    // la rivière s'arrête avant les coins des voies (embouchures)
    let along = (x - y).abs() * FRAC_1_SQRT_2;
    let river_mask = 1.0 - smooth(5800.0, 6600.0, along);
    let mut h = -RIVER_DEPTH
        * (1.0 - smooth(RIVER_HALF_WIDTH - 220.0, RIVER_HALF_WIDTH + 120.0, dr))
        * river_mask;
    let lane_flat = smooth(LANE_HALF_WIDTH, LANE_HALF_WIDTH + 400.0, dist_to_lane(x, y));
    h += ((x * 0.0011).sin() * (y * 0.0013).cos() * 9.0 + ((x + y) * 0.0023).sin() * 4.0)
        * lane_flat
        * (1.0 - (1.0 - smooth(0.0, RIVER_HALF_WIDTH + 300.0, dr)) * river_mask);
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `render.test.ts` › brouillard et terrain › rivière creusée, voies plates.
    #[test]
    fn riviere_creusee_voies_plates() {
        assert!(terrain_height(6000.0, 6000.0) < -RIVER_DEPTH * 0.9);
        assert!(terrain_height(1100.0, 6000.0).abs() < 1.0);
        assert!(terrain_height(4000.0, 4000.0).abs() < 1.0);
    }

    #[test]
    fn distances_aux_voies_et_a_la_riviere() {
        assert_eq!(dist_to_lane(2000.0, 2000.0), 0.0);
        assert!((dist_to_river(6000.0, 6000.0)).abs() < 1e-9);
        assert!((dist_to_river(6000.0, 7000.0) - 1000.0 * FRAC_1_SQRT_2).abs() < 1e-9);
        let (a, b) = (Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 10.0, y: 0.0 });
        assert_eq!(dist_to_segment(5.0, 3.0, a, b), 3.0);
        assert_eq!(dist_to_segment(-4.0, 3.0, a, b), 5.0);
        assert_eq!(dist_to_segment(1.0, 1.0, a, a), 2f64.sqrt());
    }
}
