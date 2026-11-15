//! Picking d'entités : port de `packages/client/src/render/picking.ts`.
//!
//! Intersection d'un rayon avec des cylindres verticaux simplifiés (hitbox généreuses pour faciliter le
//! clic), en coordonnées monde. Fonctions pures, sans allocation.

use bevy::math::Vec3;
use lov_shared::types::{EntityKind, EntitySnapshot};

use crate::render::RENDER_SCALE;

/// Distance le long du rayon jusqu'au cylindre vertical (axe en `(cx, cz)`, de `y = 0` à `y = h`,
/// rayon `r`), ou `None`. Gère l'entrée par le flanc et par le couvercle supérieur.
pub fn ray_cylinder(origin: Vec3, dir: Vec3, cx: f32, cz: f32, r: f32, h: f32) -> Option<f32> {
    let (ox, oz) = (origin.x - cx, origin.z - cz);
    let a = dir.x * dir.x + dir.z * dir.z;
    let mut best: Option<f32> = None;
    if a > 1e-12 {
        let b = 2.0 * (ox * dir.x + oz * dir.z);
        let c = ox * ox + oz * oz - r * r;
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let t = (-b - disc.sqrt()) / (2.0 * a);
            let y = origin.y + dir.y * t;
            if t >= 0.0 && (0.0..=h).contains(&y) {
                best = Some(t);
            }
        }
    }
    // Couvercle supérieur.
    if dir.y.abs() > 1e-12 {
        let t = (h - origin.y) / dir.y;
        if t >= 0.0 && best.is_none_or(|b| t < b) {
            let (x, z) = (ox + dir.x * t, oz + dir.z * t);
            if x * x + z * z <= r * r {
                best = Some(t);
            }
        }
    }
    best
}

fn monster_hitbox(subtype: &str) -> (f32, f32) {
    match subtype {
        "dragon" => (260.0, 300.0),
        "elder_dragon" => (300.0, 340.0),
        "leviathan" => (380.0, 420.0),
        "herald" => (260.0, 320.0),
        "blue_buff" | "red_buff" => (150.0, 220.0),
        "gromp" | "krugs" => (130.0, 160.0),
        "wolves" => (110.0, 140.0),
        "raptors" => (100.0, 130.0),
        "scuttle" => (110.0, 110.0),
        "grub" => (80.0, 90.0),
        _ => (60.0, 90.0),
    }
}

/// Hitbox `(rayon, hauteur)` en unités de jeu ; `None` pour ce qui ne se clique pas (projectiles,
/// zones).
pub fn hitbox_for(e: &EntitySnapshot) -> Option<(f32, f32)> {
    Some(match e.kind {
        EntityKind::Champion => (80.0, 200.0),
        EntityKind::Minion => match e.subtype.as_deref() {
            Some("super" | "cannon") => (75.0, 140.0),
            _ => (55.0, 110.0),
        },
        EntityKind::Tower => (140.0, 520.0),
        EntityKind::Inhibitor => (170.0, 200.0),
        EntityKind::Nexus => (260.0, 320.0),
        EntityKind::Monster => monster_hitbox(e.subtype.as_deref().unwrap_or("")),
        EntityKind::Ward => (45.0, 90.0),
        EntityKind::Projectile | EntityKind::Zone => return None,
    })
}

/// Hitbox en unités monde.
pub fn world_hitbox(e: &EntitySnapshot) -> Option<(f32, f32)> {
    hitbox_for(e).map(|(r, h)| (r * RENDER_SCALE, h * RENDER_SCALE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touche_le_flanc_et_le_couvercle() {
        // Rayon horizontal vers un cylindre de rayon 1 centré en x = 5 : entrée par le flanc en x = 4.
        let t = ray_cylinder(Vec3::new(0.0, 0.5, 0.0), Vec3::X, 5.0, 0.0, 1.0, 2.0);
        assert!((t.unwrap() - 4.0).abs() < 1e-5);
        // Rayon vertical descendant au-dessus de l'axe : entrée par le couvercle, à y = 2.
        let t = ray_cylinder(Vec3::new(5.0, 10.0, 0.0), -Vec3::Y, 5.0, 0.0, 1.0, 2.0);
        assert!((t.unwrap() - 8.0).abs() < 1e-5);
        // Rayon qui passe à côté, et rayon qui passe au-dessus.
        assert!(ray_cylinder(Vec3::new(0.0, 0.5, 3.0), Vec3::X, 5.0, 0.0, 1.0, 2.0).is_none());
        assert!(ray_cylinder(Vec3::new(0.0, 3.0, 0.0), Vec3::X, 5.0, 0.0, 1.0, 2.0).is_none());
    }

    #[test]
    fn projectiles_et_zones_ne_se_cliquent_pas() {
        let mut e = EntitySnapshot {
            id: 1,
            kind: EntityKind::Projectile,
            team: lov_shared::types::TeamOrNeutral::Red,
            x: 0.0,
            y: 0.0,
            facing: 0.0,
            hp: 1.0,
            max_hp: 1.0,
            mana: None,
            max_mana: None,
            shield: None,
            dead: None,
            subtype: None,
            owner_id: None,
            level: None,
            anim: None,
            buffs: None,
            champ: None,
        };
        assert!(hitbox_for(&e).is_none());
        e.kind = EntityKind::Tower;
        assert_eq!(hitbox_for(&e), Some((140.0, 520.0)));
    }
}
