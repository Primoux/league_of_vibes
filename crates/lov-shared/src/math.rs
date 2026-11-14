//! Fonctions mathématiques de jeu (port de `math.ts`).

use crate::types::Vec2;

pub fn dist(a: Vec2, b: Vec2) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

pub fn dist2(a: Vec2, b: Vec2) -> f64 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}

/// Équivalent de `Math.max(lo, Math.min(hi, v))` (même ordre des opérations, donc même résultat
/// si `lo > hi`).
pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    lo.max(v.min(hi))
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub fn angle_to(from: Vec2, to: Vec2) -> f64 {
    (to.y - from.y).atan2(to.x - from.x)
}

/// Avance `from` vers `to` d'au plus `step`.
pub fn move_towards(from: Vec2, to: Vec2, step: f64) -> Vec2 {
    let d = dist(from, to);
    if d <= step || d == 0.0 {
        return to;
    }
    Vec2 {
        x: from.x + ((to.x - from.x) / d) * step,
        y: from.y + ((to.y - from.y) / d) * step,
    }
}

/// Réduction de dégâts LoL : 100 / (100 + résistance) (résistance négative : 2 - 100/(100 - r)).
pub fn damage_multiplier(resist: f64) -> f64 {
    if resist >= 0.0 {
        100.0 / (100.0 + resist)
    } else {
        2.0 - 100.0 / (100.0 - resist)
    }
}

/// Valeur par niveau façon LoL : base + growth * (n-1) * (0.7025 + 0.0175 * (n-1)).
pub fn stat_at_level(base: f64, growth: f64, level: f64) -> f64 {
    let n = level - 1.0;
    base + growth * n * (0.7025 + 0.0175 * n)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn v(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }

    #[test]
    fn distances() {
        assert_eq!(dist(v(0.0, 0.0), v(3.0, 4.0)), 5.0);
        assert_eq!(dist2(v(0.0, 0.0), v(3.0, 4.0)), 25.0);
    }

    #[test]
    fn clamp_lerp_angle() {
        assert_eq!(clamp(5.0, 0.0, 3.0), 3.0);
        assert_eq!(clamp(-1.0, 0.0, 3.0), 0.0);
        assert_eq!(clamp(2.0, 0.0, 3.0), 2.0);
        assert_eq!(lerp(10.0, 20.0, 0.25), 12.5);
        assert!((angle_to(v(0.0, 0.0), v(0.0, 1.0)) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn move_towards_stops_at_target() {
        assert_eq!(move_towards(v(0.0, 0.0), v(3.0, 4.0), 10.0), v(3.0, 4.0));
        assert_eq!(move_towards(v(0.0, 0.0), v(3.0, 4.0), 2.5), v(1.5, 2.0));
        assert_eq!(move_towards(v(1.0, 1.0), v(1.0, 1.0), 0.0), v(1.0, 1.0));
    }

    #[test]
    fn damage_and_levels() {
        assert_eq!(damage_multiplier(100.0), 0.5);
        assert_eq!(damage_multiplier(0.0), 1.0);
        assert_eq!(damage_multiplier(-100.0), 1.5);
        assert_eq!(stat_at_level(100.0, 10.0, 1.0), 100.0);
        assert!((stat_at_level(100.0, 10.0, 2.0) - 107.2).abs() < 1e-9);
    }
}
