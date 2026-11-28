//! Marqueurs de clic au sol : anneau vert pour un déplacement, rouge pour une attaque ou une
//! attaque-déplacement. Portage de `packages/client/src/render/ClickMarkers.ts`.
//!
//! Même forme et même durée que le web : anneau de rayons 0,25 à 0,36, posé 0,06 au-dessus du relief,
//! qui rétrécit de ×1,6 à ×0,6 et s'efface en 0,45 s (opacité `1 − k²`). Pool fixe de six marqueurs,
//! réutilisés en tourniquet : aucune entité ni matériau créé après le démarrage.

use bevy::prelude::*;
use lov_shared::types::Vec2 as Pos;

use super::PlayerInput;
use crate::render::RENDER_SCALE;
use crate::render::map::geometry::terrain_height;

const POOL: usize = 6;
/// Durée de vie d'un marqueur (s).
pub const LIFE: f32 = 0.45;
/// Hauteur au-dessus du sol (monde) : au-dessus des décals de portée.
const LIFT: f32 = 0.06;
/// `0x5cff8a` du web.
pub const MOVE_COLOR: Color = Color::srgb(0x5c as f32 / 255.0, 1.0, 0x8a as f32 / 255.0);
/// `0xff4040` du web.
pub const ATTACK_COLOR: Color = Color::srgb(1.0, 0x40 as f32 / 255.0, 0x40 as f32 / 255.0);

/// Un ordre à marquer au sol, déposé par les entrées et consommé par [`update_markers`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClickMark {
    pub pos: Pos,
    pub attack: bool,
}

/// Un marqueur du pool.
#[derive(Component, Debug, Clone, Copy)]
pub struct Marker {
    /// Âge (s) ; `>= LIFE` : libre et caché.
    t: f32,
    material: AssetId<StandardMaterial>,
}

/// Prochain marqueur du tourniquet.
#[derive(Resource, Debug, Default)]
pub struct MarkerCursor(usize);

/// Échelle et opacité à l'âge `t` (s), `None` une fois éteint : `ClickMarkers.update` du web.
pub fn marker_look(t: f32) -> Option<(f32, f32)> {
    let k = t / LIFE;
    (k < 1.0).then_some((1.6 - k, 1.0 - k * k))
}

/// Position monde d'un marqueur : posé sur le relief, comme le web (`terrainHeight * S + 0.06`).
pub fn marker_pos(p: Pos) -> Vec3 {
    let h = terrain_height(p.x, p.y) * f64::from(RENDER_SCALE);
    Vec3::new(
        p.x as f32 * RENDER_SCALE,
        h as f32 + LIFT,
        -(p.y as f32) * RENDER_SCALE,
    )
}

pub fn spawn_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // `RingGeometry(0.25, 0.36, 24).rotateX(-π/2)` : anneau couché au sol, face vers le haut
    let mesh = meshes.add(
        Annulus::new(0.25, 0.36)
            .mesh()
            .resolution(24)
            .build()
            .rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    );
    for _ in 0..POOL {
        let material = materials.add(StandardMaterial {
            base_color: MOVE_COLOR.with_alpha(0.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            Name::new("Marqueur de clic"),
            Marker {
                t: LIFE,
                material: material.id(),
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            bevy::light::NotShadowCaster,
        ));
    }
}

/// Pose les marqueurs des ordres de l'image, puis fait vieillir ceux qui sont visibles.
pub fn update_markers(
    time: Res<Time>,
    mut input: ResMut<PlayerInput>,
    mut cursor: ResMut<MarkerCursor>,
    mut markers: Query<(&mut Marker, &mut Transform, &mut Visibility)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    // pris puis rendu : la liste garde sa capacité, sans allocation
    let mut clicks = std::mem::take(&mut input.clicks);
    for click in clicks.drain(..) {
        // tourniquet sur le pool, sans allocation (six marqueurs)
        let Some((mut m, mut tf, mut vis)) = markers.iter_mut().nth(cursor.0) else {
            break;
        };
        cursor.0 = (cursor.0 + 1) % POOL;
        m.t = -dt; // remis à 0 par le vieillissement ci-dessous
        tf.translation = marker_pos(click.pos);
        *vis = Visibility::Inherited;
        if let Some(mut mat) = materials.get_mut(m.material) {
            mat.base_color = if click.attack {
                ATTACK_COLOR
            } else {
                MOVE_COLOR
            };
        }
    }
    input.clicks = clicks;
    for (mut m, mut tf, mut vis) in &mut markers {
        if m.t >= LIFE {
            continue;
        }
        m.t += dt;
        match marker_look(m.t) {
            Some((scale, alpha)) => {
                tf.scale = Vec3::splat(scale);
                if let Some(mut mat) = materials.get_mut(m.material) {
                    mat.base_color.set_alpha(alpha);
                }
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_marqueur_retrecit_et_s_efface_en_0_45_s() {
        assert_eq!(marker_look(0.0), Some((1.6, 1.0)));
        let (s, a) = marker_look(LIFE / 2.0).expect("vivant");
        assert!((s - 1.1).abs() < 1e-6 && (a - 0.75).abs() < 1e-6);
        assert_eq!(marker_look(LIFE), None);
    }

    #[test]
    fn le_marqueur_est_pose_sur_le_relief() {
        let river = marker_pos(Pos {
            x: 6000.0,
            y: 6000.0,
        });
        assert!(river.y < -0.25, "{river}");
        let lane = marker_pos(Pos {
            x: 1100.0,
            y: 6000.0,
        });
        assert!((lane.y - LIFT).abs() < 0.01, "{lane}");
        assert_eq!((lane.x, lane.z), (11.0, -60.0));
    }
}
