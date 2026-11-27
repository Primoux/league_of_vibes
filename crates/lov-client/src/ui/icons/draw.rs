//! Icônes du HUD dessinées sur le processeur : sorts, objets, sorts d'invocateur et glyphes.
//!
//! Portage de `AbilityIcon`, `ItemIcon`, `SummonerIcon` et `Glyph` (`packages/client/src/ui/icons/Icons.tsx`)
//! avec le moteur de `raster` : mêmes `viewBox`, dégradés, rayons, anneaux pointillés, halo flou et
//! contours sous le remplissage. Chaque icône est dessinée une fois puis mise en cache ([`IconCache`]).
//! Les glyphes sont blancs : l'interface les teinte (`ImageNode::color`), comme `currentColor`.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use bevy::math::Affine2;
use bevy::prelude::*;
use lov_shared::schema::{AbilityDef, AbilityDefSlot, ChampionDef, ItemCategory, ItemDef};
use lov_shared::types::{DamageType, SummonerId};

use super::glyphs::{ability_glyph, item_glyph, item_tint, item_variation};
use super::paths::{CATEGORY_COLOR, CLASS_EMBLEM, MISC, SUMMONER_ART};
use super::raster::{Canvas, Path, Rgba, blur, css, dilate, rgb, stops};
use crate::ui::logic::format::hash_string;

/// Côté des icônes de sorts et d'objets (px) : net jusqu'à ~90 px affichés.
pub const ICON_PX: usize = 96;
/// Côté des glyphes (px).
pub const GLYPH_PX: usize = 64;

const NIGHT: u32 = 0x05060c;

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

fn with_alpha(c: Rgba, a: f32) -> Rgba {
    [c[0], c[1], c[2], a]
}

/// Dégradé radial (`radialGradient` en `objectBoundingBox` sur un carré de côté `view`).
fn radial(view: f32, cx: f32, cy: f32, r: f32, list: &[(f32, Rgba)]) -> impl Fn(Vec2) -> Rgba + '_ {
    let c = Vec2::new(cx, cy) * view;
    let r = r * view;
    move |p| stops(list, (p - c).length() / r)
}

/// Boîte englobante d'un tracé.
fn bounds(p: &Path) -> (Vec2, Vec2) {
    let mut lo = Vec2::splat(f32::INFINITY);
    let mut hi = Vec2::splat(f32::NEG_INFINITY);
    for v in p.polys.iter().flatten() {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    (lo, hi)
}

/// `filter: hue-rotate(deg)` du CSS (matrice du W3C), sur une couleur sRGB.
pub fn hue_rotate(c: Rgba, deg: f32) -> Rgba {
    let (s, k) = (deg * PI / 180.0).sin_cos();
    let m = [
        [
            0.213 + k * 0.787 - s * 0.213,
            0.715 - k * 0.715 - s * 0.715,
            0.072 - k * 0.072 + s * 0.928,
        ],
        [
            0.213 - k * 0.213 + s * 0.143,
            0.715 + k * 0.285 + s * 0.140,
            0.072 - k * 0.072 - s * 0.283,
        ],
        [
            0.213 - k * 0.213 - s * 0.787,
            0.715 - k * 0.715 + s * 0.715,
            0.072 + k * 0.928 + s * 0.072,
        ],
    ];
    let o: [f32; 3] =
        std::array::from_fn(|i| (m[i][0] * c[0] + m[i][1] * c[1] + m[i][2] * c[2]).clamp(0.0, 1.0));
    [o[0], o[1], o[2], c[3]]
}

/// Glyphe de 24 unités posé par `t`, avec contour sombre sous le remplissage (`paint-order: stroke`),
/// rempli par `fill` (fonction du point dans le `viewBox`).
fn glyph_with_stroke(
    cv: &mut Canvas,
    d: &str,
    t: Affine2,
    stroke_w: f32,
    fill: impl Fn(Vec2) -> Rgba,
) {
    let path = Path::parse(d);
    let mask = cv.mask(&path, t);
    let scale = cv.scale * t.matrix2.x_axis.length();
    let outline = dilate(&mask, cv.w, cv.h, stroke_w * scale / 2.0);
    cv.paint(&outline, 1.0, |_| rgb(0x0a0a10, 1.0));
    cv.paint(&mask, 1.0, fill);
}

/// Icône d'un sort de champion (`AbilityIcon`, `viewBox` 48).
pub fn ability_icon(a: &AbilityDef, champ: Option<&ChampionDef>) -> Image {
    let (primary, secondary, accent) = champ.map_or(
        (
            css("#888888", 1.0),
            css("#222222", 1.0),
            css("#eeeeee", 1.0),
        ),
        |c| {
            (
                css(&c.palette.primary, 1.0),
                css(&c.palette.secondary, 1.0),
                css(&c.palette.accent, 1.0),
            )
        },
    );
    let tint = match a.damage.as_ref().map(|d| d.damage_type) {
        Some(DamageType::Physical) => css("#ffb36b", 1.0),
        Some(DamageType::Magic) => css("#d2a8ff", 1.0),
        Some(DamageType::True) => css("#ffffff", 1.0),
        None => accent,
    };
    let h = hash_string(&a.id);
    let mut cv = Canvas::new(ICON_PX, 48.0);
    let full = cv.full();
    let bg = [(0.0, primary), (0.55, secondary), (1.0, rgb(NIGHT, 1.0))];
    cv.paint(&full, 1.0, radial(48.0, 0.3, 0.25, 0.95, &bg));
    // rayons
    let ang = (h % 360) as f32 * PI / 180.0;
    let mut rays = String::new();
    for i in 0..6 {
        let a0 = ang + i as f32 * PI / 3.0;
        let a1 = a0 + 0.22;
        rays.push_str(&format!(
            "M24 24 L{} {} L{} {} Z ",
            24.0 + a0.cos() * 40.0,
            24.0 + a0.sin() * 40.0,
            24.0 + a1.cos() * 40.0,
            24.0 + a1.sin() * 40.0
        ));
    }
    let rays = cv.mask(&Path::parse(&rays), Affine2::IDENTITY);
    cv.paint(&rays, 0.1, |_| accent);
    // halo
    let glow = [
        (0.0, with_alpha(tint, 0.7)),
        (0.6, with_alpha(tint, 0.12)),
        (1.0, with_alpha(tint, 0.0)),
    ];
    let disc = cv.ring(Vec2::splat(24.0), 10.0, 20.0, None);
    cv.paint(&disc, 1.0, |p| {
        stops(&glow, (p - Vec2::splat(24.0)).length() / 20.0)
    });
    // anneau pointillé
    let ring = cv.ring(
        Vec2::splat(24.0),
        13.0 + (h % 5) as f32,
        0.7,
        Some((2.0, 3.0)),
    );
    cv.paint(&ring, 0.22, |_| accent);
    // glyphe : halo flou, contour, dégradé vertical
    let t = Affine2::from_translation(Vec2::splat(7.0)) * Affine2::from_scale(Vec2::splat(1.4167));
    let path = Path::parse(ability_glyph(a));
    let (lo, hi) = bounds(&path.transformed(t));
    let grad = [(0.0, rgb(0xffffff, 1.0)), (0.55, accent), (1.0, tint)];
    let fill = move |p: Vec2| stops(&grad, (p.y - lo.y) / (hi.y - lo.y).max(1e-3));
    let mask = cv.mask(&path, t);
    let halo = blur(&mask, cv.w, cv.h, 1.6 * 1.4167 * cv.scale);
    cv.paint(&halo, 1.0, fill);
    glyph_with_stroke(&mut cv, ability_glyph(a), t, 0.7, fill);
    // reflet
    let shine = cv.mask(
        &Path::parse("M0 0 H48 V16 C32 10 16 10 0 16 Z"),
        Affine2::IDENTITY,
    );
    cv.paint(&shine, 0.08, |_| rgb(0xffffff, 1.0));
    if a.slot == AbilityDefSlot::R {
        let corners = cv.mask(
            &Path::parse(
                "M0 0 H12 L0 12 Z M48 48 H36 L48 36 Z M48 0 V12 L36 0 Z M0 48 V36 L12 48 Z",
            ),
            Affine2::IDENTITY,
        );
        cv.paint(&corners, 0.75, |_| tint);
    }
    cv.into_image()
}

fn category_key(c: ItemCategory) -> &'static str {
    match c {
        ItemCategory::Starter => "starter",
        ItemCategory::Consumable => "consumable",
        ItemCategory::Boots => "boots",
        ItemCategory::Basic => "basic",
        ItemCategory::Epic => "epic",
        ItemCategory::Legendary => "legendary",
        ItemCategory::Trinket => "trinket",
    }
}

/// Couleurs (claire, sombre) de la catégorie d'un objet.
pub fn category_colors(c: ItemCategory) -> (u32, u32) {
    let key = category_key(c);
    CATEGORY_COLOR
        .iter()
        .find(|(k, _, _)| *k == key)
        .map_or((0x7f8ea3, 0x1a1f29), |(_, a, b)| (*a, *b))
}

/// Icône d'un objet (`ItemIcon`, `viewBox` 40).
pub fn item_icon(item: &ItemDef) -> Image {
    let (c1, c2) = category_colors(item.category);
    let tint = rgb(item_tint(item), 1.0);
    let v = item_variation(&item.id);
    let mut cv = Canvas::new(ICON_PX, 40.0);
    let full = cv.full();
    let bg = [
        (0.0, hue_rotate(rgb(c1, 1.0), v.hue as f32)),
        (1.0, rgb(c2, 1.0)),
    ];
    cv.paint(
        &full,
        1.0,
        radial(40.0, v.light_x as f32 / 100.0, 0.28, 0.9, &bg),
    );
    let ring = cv.ring(Vec2::splat(20.0), 9.0 + v.ring as f32, 0.6, None);
    cv.paint(&ring, 0.14, |_| tint);
    let ring2 = cv.ring(
        Vec2::splat(20.0),
        15.0 + v.ring as f32 / 2.0,
        0.6,
        Some((2.0, 3.0)),
    );
    cv.paint(&ring2, 0.14, |_| tint);
    let glow = [(0.0, with_alpha(tint, 0.45)), (1.0, with_alpha(tint, 0.0))];
    let disc = cv.ring(Vec2::splat(20.0), 7.5, 15.0, None);
    cv.paint(&disc, 1.0, |p| {
        stops(&glow, (p - Vec2::splat(20.0)).length() / 15.0)
    });
    let tilt = v.tilt as f32 * PI / 180.0;
    let t = Affine2::from_translation(Vec2::splat(6.0))
        * Affine2::from_scale(Vec2::splat(1.1667))
        * Affine2::from_translation(Vec2::splat(12.0))
        * Affine2::from_angle(tilt)
        * Affine2::from_translation(Vec2::splat(-12.0));
    glyph_with_stroke(&mut cv, item_glyph(item), t, 0.9, |_| tint);
    if item.category == ItemCategory::Legendary {
        let x = v.gem_x as f32;
        let gem = format!("M{x} 4.5 L{} 7 L{x} 9.5 L{} 7 Z", x + 2.5, x - 2.5);
        glyph_with_stroke(&mut cv, &gem, Affine2::IDENTITY, 0.6, |_| rgb(v.gem, 1.0));
    }
    let shine = cv.mask(
        &Path::parse("M0 0 H40 V12 C27 8 13 8 0 12 Z"),
        Affine2::IDENTITY,
    );
    cv.paint(&shine, 0.07, |_| rgb(0xffffff, 1.0));
    let frame = cv.mask(
        &Path::parse("M0 0 H40 V40 H0 Z M1.5 1.5 V38.5 H38.5 V1.5 Z"),
        Affine2::IDENTITY,
    );
    cv.paint(&frame, 0.9, |_| rgb(c1, 1.0));
    cv.into_image()
}

/// Nom d'un sort d'invocateur dans les tables d'icônes.
pub fn summoner_key(id: SummonerId) -> &'static str {
    match id {
        SummonerId::Flash => "flash",
        SummonerId::Ignite => "ignite",
        SummonerId::Heal => "heal",
        SummonerId::Barrier => "barrier",
        SummonerId::Exhaust => "exhaust",
        SummonerId::Teleport => "teleport",
        SummonerId::Smite => "smite",
        SummonerId::Ghost => "ghost",
        SummonerId::Cleanse => "cleanse",
    }
}

/// Icône d'un sort d'invocateur (`SummonerIcon`, `viewBox` 32).
pub fn summoner_icon(id: SummonerId) -> Image {
    let key = summoner_key(id);
    let (glyph, c1, c2) = SUMMONER_ART.iter().find(|(k, _, _, _)| *k == key).map_or(
        (SUMMONER_ART[0].1, SUMMONER_ART[0].2, SUMMONER_ART[0].3),
        |(_, g, a, b)| (*g, *a, *b),
    );
    let mut cv = Canvas::new(ICON_PX, 32.0);
    let full = cv.full();
    // `stop-opacity` 0,55 sur la couleur claire : mélangée au fond sombre, comme le navigateur sur un fond opaque
    let bg = [
        (0.0, mix(rgb(c2, 1.0), rgb(c1, 1.0), 0.55)),
        (1.0, rgb(c2, 1.0)),
    ];
    cv.paint(&full, 1.0, radial(32.0, 0.5, 0.4, 0.75, &bg));
    glyph_with_stroke(
        &mut cv,
        glyph,
        Affine2::from_translation(Vec2::splat(4.0)),
        0.8,
        |_| rgb(c1, 1.0),
    );
    cv.into_image()
}

/// Tracé d'un glyphe divers (`Glyph`), le nexus si le nom est inconnu.
pub fn misc_path(name: &str) -> &'static str {
    MISC.iter()
        .find(|(k, _)| *k == name)
        .or_else(|| MISC.iter().find(|(k, _)| *k == "nexus"))
        .map_or("", |(_, d)| *d)
}

/// Glyphe blanc de 24 unités (`Glyph`), à teinter. `d` : tracé (voir [`misc_path`]).
pub fn glyph_image(d: &str) -> Image {
    let mut cv = Canvas::new(GLYPH_PX, 24.0);
    let mask = cv.mask(&Path::parse(d), Affine2::IDENTITY);
    cv.paint(&mask, 1.0, |_| rgb(0xffffff, 1.0));
    cv.into_image()
}

/// Emblème de classe (`ClassEmblem`).
pub fn emblem_path(class: &str) -> &'static str {
    CLASS_EMBLEM
        .iter()
        .find(|(k, _)| *k == class)
        .or_else(|| CLASS_EMBLEM.iter().find(|(k, _)| *k == "mage"))
        .map_or("", |(_, d)| *d)
}

/// Disque de progression (`conic-gradient` du balayage de recharge) : la part `f` (0..1) de la face,
/// dans le sens des aiguilles d'une montre depuis midi, sombre ; le reste voilé.
pub fn sweep_image(f: f32, size: usize) -> Image {
    let mut cv = Canvas::new(size, 1.0);
    let full = cv.full();
    let f = f.clamp(0.0, 1.0);
    cv.paint(&full, 1.0, |p| {
        let d = p - Vec2::splat(0.5);
        // angle depuis midi, sens horaire (y vers le bas)
        let a = d.x.atan2(-d.y).rem_euclid(TAU) / TAU;
        if a < f {
            [0.0, 0.0, 0.0, 0.74]
        } else {
            [0.0, 0.0, 0.0, 0.25]
        }
    });
    cv.into_image()
}

/// Icônes déjà dessinées, par clé.
#[derive(Resource, Default)]
pub struct IconCache {
    map: HashMap<String, Handle<Image>>,
}

impl IconCache {
    fn get_or(
        &mut self,
        images: &mut Assets<Image>,
        key: String,
        make: impl FnOnce() -> Image,
    ) -> Handle<Image> {
        self.map
            .entry(key)
            .or_insert_with(|| images.add(make()))
            .clone()
    }

    pub fn ability(
        &mut self,
        images: &mut Assets<Image>,
        a: &AbilityDef,
        champ: Option<&ChampionDef>,
    ) -> Handle<Image> {
        self.get_or(images, format!("a:{}", a.id), || ability_icon(a, champ))
    }

    pub fn item(&mut self, images: &mut Assets<Image>, item: &ItemDef) -> Handle<Image> {
        self.get_or(images, format!("i:{}", item.id), || item_icon(item))
    }

    pub fn summoner(&mut self, images: &mut Assets<Image>, id: SummonerId) -> Handle<Image> {
        self.get_or(images, format!("s:{}", summoner_key(id)), || {
            summoner_icon(id)
        })
    }

    /// Glyphe divers par nom (`gold`, `tower`, `skull`…).
    pub fn glyph(&mut self, images: &mut Assets<Image>, name: &str) -> Handle<Image> {
        self.get_or(images, format!("g:{name}"), || glyph_image(misc_path(name)))
    }

    /// Glyphe d'un tracé quelconque, mis en cache sous `key`.
    pub fn path(&mut self, images: &mut Assets<Image>, key: &str, d: &str) -> Handle<Image> {
        self.get_or(images, format!("p:{key}"), || glyph_image(d))
    }

    /// Balayage de recharge quantifié en 64 pas.
    pub fn sweep(&mut self, images: &mut Assets<Image>, f: f32) -> Handle<Image> {
        let step = (f.clamp(0.0, 1.0) * 64.0).round() as u32;
        self.get_or(images, format!("w:{step}"), || {
            sweep_image(step as f32 / 64.0, 48)
        })
    }
}

#[cfg(test)]
mod tests {
    use lov_shared::data::champions::get_champion;
    use lov_shared::data::items::ITEM_LIST;
    use lov_shared::data::summoners::SUMMONERS;
    use lov_shared::types::AbilitySlot;

    use super::*;

    fn alpha_at(img: &Image, x: u32, y: u32) -> u8 {
        let w = img.width();
        img.data
            .as_ref()
            .map_or(0, |d| d[((y * w + x) * 4 + 3) as usize])
    }

    #[test]
    fn icones_opaques_et_ult_marque() {
        let Some(c) = get_champion("ilyra") else {
            panic!("ilyra");
        };
        let q = ability_icon(c.abilities.get(AbilitySlot::Q), Some(c));
        let r = ability_icon(c.abilities.get(AbilitySlot::R), Some(c));
        assert_eq!((q.width(), q.height()), (ICON_PX as u32, ICON_PX as u32));
        assert_eq!(alpha_at(&q, 2, 90), 255, "fond opaque");
        // coin de l'ultime : teinte vive, pas le fond sombre
        let px = |img: &Image| img.data.as_ref().map_or(0, |d| d[(2 * ICON_PX + 2) * 4]);
        assert!(px(&r) > px(&q), "coins de l'ultime");
    }

    #[test]
    fn chaque_objet_et_invocateur_se_dessine() {
        for item in ITEM_LIST.iter().take(8) {
            let img = item_icon(item);
            assert_eq!(alpha_at(&img, 48, 48), 255, "{}", item.id);
        }
        for s in SUMMONERS.iter() {
            assert_eq!(summoner_icon(s.id).width(), ICON_PX as u32);
        }
    }

    #[test]
    fn glyphe_blanc_transparent_autour() {
        let g = glyph_image(misc_path("gold"));
        assert_eq!(alpha_at(&g, 0, 0), 0);
        assert!(alpha_at(&g, 32, 40) > 200);
    }

    #[test]
    fn balayage_de_recharge() {
        let img = sweep_image(0.25, 48);
        let a = |x, y| alpha_at(&img, x, y);
        assert!(a(40, 10) > a(10, 10), "quart haut droit assombri");
        assert!(
            (hue_rotate([0.5, 0.5, 0.5, 1.0], 90.0)[0] - 0.5).abs() < 1e-3,
            "gris inchangé"
        );
    }
}
