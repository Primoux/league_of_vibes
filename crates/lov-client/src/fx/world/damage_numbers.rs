//! Chiffres flottants (dégâts, soins, or) au-dessus des entités : montée, grossissement à l'apparition,
//! fondu, légère dérive.
//!
//! Portage de `packages/client/src/fx/world/damageNumbers.ts`. Le web dessine un quad instancié par glyphe
//! avec un atlas canvas ; ici chaque chiffre est un texte de l'interface placé chaque image à la projection
//! écran de son ancre (comme les étiquettes des barres de vie), avec une ombre portée pour le contour
//! sombre. Les textes sont recyclés en anneau : aucune entité créée une fois l'anneau plein. Mêmes courbes
//! que le vertex shader du web (montée, pop de taille, fondu, dérive).

use bevy::prelude::*;

use crate::fx::core::runtime::Fx;
use crate::fx::effects::primitives::rand_range;
use crate::render::camera::MainCamera;
use crate::ui::UiFonts;

/// Textes simultanés au plus (le web : 768 glyphes, soit ~200 chiffres).
const CAPACITY: usize = 128;
/// Hauteur d'un glyphe dessiné / hauteur de sa cellule (`CELL_H * 0.78` du web).
const GLYPH_FILL: f32 = 0.78;

#[derive(Clone, Default)]
struct Slot {
    entity: Option<Entity>,
    anchor: Vec3,
    born: f32,
    life: f32,
    dx: f32,
    dz: f32,
    size: f32,
    color: Vec3,
    chars: usize,
    /// Taille de police posée (px), pour ne relancer la mise en page qu'au besoin.
    font_px: f32,
    alive: bool,
}

/// Chiffres flottants (ressource).
#[derive(Resource)]
pub struct FloatingNumbers {
    slots: Vec<Slot>,
    next: usize,
    clock: f32,
}

impl Default for FloatingNumbers {
    fn default() -> Self {
        Self {
            slots: vec![Slot::default(); CAPACITY],
            next: 0,
            clock: 0.0,
        }
    }
}

/// Montée (monde) à la progression `t`.
pub fn rise(t: f32) -> f32 {
    0.9 * (1.0 - (1.0 - (t * 1.6).min(1.0)).powi(3))
}

/// Opacité à la progression `t` (`1 - smoothstep(0.65, 1, t)`).
pub fn alpha(t: f32) -> f32 {
    let x = ((t - 0.65) / 0.35).clamp(0.0, 1.0);
    1.0 - x * x * (3.0 - 2.0 * x)
}

/// Multiplicateur de taille à l'âge `age` (s) : pop à l'apparition.
pub fn pop(age: f32) -> f32 {
    1.0 + 0.7 * (-age * 14.0).exp()
}

impl FloatingNumbers {
    /// Affiche `text` au-dessus de `pos` (monde), de taille `size` (monde) pendant `life` s.
    pub fn spawn(fx: &mut Fx, pos: Vec3, text: &str, color: Vec3, size: f32, life: f32) {
        let font = fx
            .world()
            .get_resource::<UiFonts>()
            .map(|f| f.text(16.0, 900));
        let world = fx.world();
        let mut numbers = world.get_resource_or_init::<FloatingNumbers>();
        let i = numbers.next;
        numbers.next = (i + 1) % CAPACITY;
        let clock = numbers.clock;
        let entity = numbers.slots[i].entity;
        numbers.slots[i] = Slot {
            entity,
            anchor: pos,
            born: clock,
            life,
            dx: rand_range(-0.35, 0.35),
            dz: rand_range(-0.15, 0.15),
            size,
            color,
            chars: text.chars().count(),
            font_px: 0.0,
            alive: true,
        };
        let reused = entity.filter(|e| world.get_entity(*e).is_ok());
        if let Some(e) = reused {
            if let Some(mut t) = world.get_mut::<Text>(e) {
                text.clone_into(&mut t.0);
            }
            // il reprend sa place, sa taille et sa couleur à la prochaine mise à jour
            return;
        }
        let e = world
            .spawn((
                Name::new("fx-number"),
                Text::new(text),
                font.unwrap_or_else(|| TextFont::from_font_size(16.0)),
                TextColor(Color::NONE),
                TextShadow {
                    offset: Vec2::splat(2.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.85),
                },
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                GlobalZIndex(-5),
                Visibility::Hidden,
                Pickable::IGNORE,
            ))
            .id();
        world.resource_mut::<FloatingNumbers>().slots[i].entity = Some(e);
    }

    /// Chiffres visibles.
    pub fn alive(&self) -> usize {
        self.slots.iter().filter(|s| s.alive).count()
    }
}

/// Avance et place les chiffres (après les FX).
#[allow(clippy::type_complexity)]
pub fn update_numbers(
    time: Res<Time>,
    numbers: Option<ResMut<FloatingNumbers>>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut q: Query<(
        &mut Node,
        &mut TextFont,
        &mut TextColor,
        &mut TextShadow,
        &mut Visibility,
    )>,
) {
    let Some(mut numbers) = numbers else { return };
    numbers.clock += time.delta_secs().min(0.1);
    let clock = numbers.clock;
    let cam = camera.iter().next();
    for s in &mut numbers.slots {
        if !s.alive {
            continue;
        }
        let Some(Ok((mut node, mut font, mut color, mut shadow, mut vis))) =
            s.entity.map(|e| q.get_mut(e))
        else {
            continue;
        };
        let age = clock - s.born;
        let t = age / s.life;
        if t > 1.0 {
            s.alive = false;
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        let p = s.anchor + Vec3::new(s.dx * t, rise(t), s.dz * t);
        let placed = cam.and_then(|(camera, tf)| {
            let at = camera.world_to_viewport(tf, p).ok()?;
            // pixels par unité monde à cette profondeur
            let side = camera.world_to_viewport(tf, p + tf.right() * 1.0).ok()?;
            Some((at, (side - at).length()))
        });
        let Some((at, ppu)) = placed else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let px = (s.size * pop(age) * ppu * GLYPH_FILL).max(6.0);
        if (px - s.font_px).abs() > 0.5 {
            s.font_px = px;
            font.font_size = px.into();
        }
        // centré sur l'ancre (glyphes à chasse ~0,62 em, comme les cellules du web)
        let w = s.chars as f32 * px * 0.62;
        node.left = Val::Px(at.x - w / 2.0);
        node.top = Val::Px(at.y - px * 0.6);
        let a = alpha(t);
        color.0 = Color::LinearRgba(LinearRgba::new(s.color.x, s.color.y, s.color.z, a));
        shadow.color = Color::srgba(0.0, 0.0, 0.0, 0.85 * a);
        vis.set_if_neq(Visibility::Inherited);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn courbes_du_shader_web() {
        assert_eq!(rise(0.0), 0.0);
        assert!((rise(1.0) - 0.9).abs() < 1e-6);
        assert!((rise(0.7) - 0.9).abs() < 1e-6, "montée finie à t = 1/1,6");
        assert_eq!(alpha(0.5), 1.0);
        assert_eq!(alpha(1.0), 0.0);
        assert!((pop(0.0) - 1.7).abs() < 1e-6);
        assert!(pop(1.0) < 1.001);
    }

    #[test]
    fn anneau_recycle_les_textes() {
        let mut w = crate::fx::effects::primitives::tests::fx_world();
        let mut fx = Fx::new(&mut w);
        for i in 0..(CAPACITY + 10) {
            FloatingNumbers::spawn(&mut fx, Vec3::ZERO, &i.to_string(), Vec3::ONE, 0.4, 0.9);
        }
        assert_eq!(w.resource::<FloatingNumbers>().alive(), CAPACITY);
        let n = w.query::<&Text>().iter(&w).count();
        assert_eq!(n, CAPACITY, "aucun texte au-delà de l'anneau");
    }
}
