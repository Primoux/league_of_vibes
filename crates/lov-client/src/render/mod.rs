//! Rendu 3D : caméra, vues d'entités, carte.

use bevy::prelude::*;

pub mod camera;
pub mod entities;
pub mod map;

/// 1 unité de jeu = 0,01 unité 3D (`RENDER_SCALE` de `packages/shared/src/constants.ts`).
pub const RENDER_SCALE: f32 = 0.01;

/// Jeu `(x, y)` → monde `(x·S, h, −y·S)`, comme `toWorld` du client web.
pub fn to_world(p: Vec2, h: f32) -> Vec3 {
    Vec3::new(p.x * RENDER_SCALE, h, -p.y * RENDER_SCALE)
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
}
