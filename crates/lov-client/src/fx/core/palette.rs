//! Couleurs des FX : palette du champion (`palette` dans ses données) ou couleur propre au sort
//! d'invocateur, et couleurs dérivées prêtes à l'emploi (`FxColors`).
//!
//! Portage de `packages/client/src/fx/core/palette.ts` et de `makeVivid` / `colors` (`FxRuntime.ts`).
//! three.js 0.187 gère les couleurs : `new THREE.Color('#hex')` est linéaire, et `getHSL` / `setHSL`
//! travaillent sur les composantes linéaires. On fait de même, pour choisir exactement les mêmes teintes.

use bevy::math::Vec3;
use lov_shared::data::champions::get_champion;

use crate::fx::keys::champion_of_key;

/// Couleurs d'un FX, en sRGB `#rrggbb` (`FxPalette`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxPalette {
    pub primary: &'static str,
    pub secondary: &'static str,
    pub accent: &'static str,
}

const fn pal(primary: &'static str, secondary: &'static str, accent: &'static str) -> FxPalette {
    FxPalette {
        primary,
        secondary,
        accent,
    }
}

const SUMMONER_PALETTES: [(&str, FxPalette); 9] = [
    ("sum_flash", pal("#ffe27a", "#c98a1a", "#ffffff")),
    ("sum_ignite", pal("#ff7a1a", "#8a1c00", "#ffd36b")),
    ("sum_heal", pal("#7dffa0", "#1f8a4c", "#e8fff0")),
    ("sum_barrier", pal("#ffd970", "#a6761d", "#fff6d6")),
    ("sum_exhaust", pal("#9a6bff", "#3a2266", "#d8c9ff")),
    ("sum_teleport", pal("#c46bff", "#4b1f7a", "#f0d9ff")),
    ("sum_smite", pal("#ffd34d", "#7a5a10", "#fffbe6")),
    ("sum_ghost", pal("#8ff2ff", "#2a6b7a", "#e6fdff")),
    ("sum_cleanse", pal("#a8f0ff", "#3b7f99", "#ffffff")),
];

pub const DEFAULT_PALETTE: FxPalette = pal("#ffe9b0", "#7a5a2a", "#ffffff");

/// Palette d'une fxKey : celle du champion propriétaire, sinon celle du sort d'invocateur, sinon la
/// palette par défaut.
pub fn palette_for_key(key: &str) -> FxPalette {
    if let Some(c) = champion_of_key(key).and_then(get_champion) {
        return pal(
            c.palette.primary.as_str(),
            c.palette.secondary.as_str(),
            c.palette.accent.as_str(),
        );
    }
    SUMMONER_PALETTES
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(DEFAULT_PALETTE, |(_, p)| *p)
}

/// `#rrggbb` → [r, g, b] en espace sRGB 0..1 (noir si illisible).
pub fn hex_to_rgb(hex: &str) -> [f32; 3] {
    let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    [
        ((n >> 16) & 255) as f32 / 255.0,
        ((n >> 8) & 255) as f32 / 255.0,
        (n & 255) as f32 / 255.0,
    ]
}

/// Conversion sRGB → linéaire (les shaders et le bloom travaillent en linéaire).
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// `new THREE.Color('#rrggbb')` : couleur linéaire.
pub fn linear(hex: &str) -> Vec3 {
    let [r, g, b] = hex_to_rgb(hex);
    Vec3::new(srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b))
}

/// Teinte, saturation, luminosité (0..1) d'une couleur, comme `Color.getHSL`.
pub fn to_hsl(c: Vec3) -> [f32; 3] {
    let (r, g, b) = (c.x, c.y, c.z);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (min + max) / 2.0;
    if min == max {
        return [0.0, 0.0, l];
    }
    let d = max - min;
    let s = if l <= 0.5 {
        d / (max + min)
    } else {
        d / (2.0 - max - min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    [h / 6.0, s, l]
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 0.5 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * 6.0 * (2.0 / 3.0 - t)
    } else {
        p
    }
}

/// Couleur depuis teinte, saturation, luminosité, comme `Color.setHSL`.
pub fn from_hsl(h: f32, s: f32, l: f32) -> Vec3 {
    let h = h.rem_euclid(1.0);
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s == 0.0 {
        return Vec3::splat(l);
    }
    let p = if l <= 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let q = 2.0 * l - p;
    Vec3::new(
        hue_to_rgb(q, p, h + 1.0 / 3.0),
        hue_to_rgb(q, p, h),
        hue_to_rgb(q, p, h - 1.0 / 3.0),
    )
}

/// Score de « lisibilité » d'une couleur : saturée et de luminosité moyenne-haute.
fn vivid_score(c: Vec3) -> f32 {
    let [_, s, l] = to_hsl(c);
    s * (1.0 - (l - 0.6).abs())
}

/// Couleur « signature » lisible : la plus vive entre primaire et accent, éclaircie si besoin.
pub fn make_vivid(primary: Vec3, accent: Vec3) -> Vec3 {
    let base = if vivid_score(accent) > vivid_score(primary) {
        accent
    } else {
        primary
    };
    let [h, s, l] = to_hsl(base);
    from_hsl(h, s.max(0.35), l.max(0.58))
}

/// Couleurs prêtes à l'emploi (linéaires) d'une palette ; `*_hdr` = surexposées pour le bloom.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FxColors {
    pub primary: Vec3,
    pub secondary: Vec3,
    pub accent: Vec3,
    pub primary_hdr: Vec3,
    pub accent_hdr: Vec3,
    /// Indispensable pour les décals additifs : une palette sombre (Nyx, Grondak) serait invisible sur
    /// l'herbe.
    pub vivid: Vec3,
    pub vivid_hdr: Vec3,
}

impl FxColors {
    pub fn of(p: FxPalette) -> Self {
        let primary = linear(p.primary);
        let accent = linear(p.accent);
        let vivid = make_vivid(primary, accent);
        Self {
            primary,
            secondary: linear(p.secondary),
            accent,
            primary_hdr: primary * 2.6,
            accent_hdr: accent * 2.2,
            vivid,
            vivid_hdr: vivid * 2.4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_par_cle() {
        assert_eq!(palette_for_key("ilyra_q").primary, "#8fd3ff");
        assert_eq!(palette_for_key("sum_ignite").primary, "#ff7a1a");
        assert_eq!(palette_for_key("inconnu"), DEFAULT_PALETTE);
    }

    /// `couleur vive` de `bus-keys.test.ts`.
    #[test]
    fn accent_pour_les_palettes_sombres_et_luminosite_minimale() {
        let pick = |id: &str| {
            let p = palette_for_key(&format!("{id}_q"));
            to_hsl(make_vivid(linear(p.primary), linear(p.accent)))
        };
        let accent_hue = |hex: &str| to_hsl(linear(hex))[0];
        let close = |a: f32, b: f32| (a - b).abs() < 0.005;
        assert!(
            close(pick("grondak")[0], accent_hue("#ffb347")),
            "brun terne → accent orange"
        );
        assert!(
            close(pick("nyx")[0], accent_hue("#ff2e63")),
            "quasi-noir → accent rose"
        );
        assert!(
            close(pick("ilyra")[0], accent_hue("#8fd3ff")),
            "primaire déjà vive"
        );
        for id in ["grondak", "nyx", "corvin", "tharox"] {
            assert!(pick(id)[2] >= 0.579, "{id}");
        }
    }

    #[test]
    fn hsl_aller_retour() {
        for c in [
            Vec3::new(0.2, 0.5, 0.9),
            Vec3::new(0.9, 0.1, 0.3),
            Vec3::splat(0.4),
        ] {
            let [h, s, l] = to_hsl(c);
            assert!((from_hsl(h, s, l) - c).abs().max_element() < 1e-5);
        }
    }

    #[test]
    fn couleurs_derivees() {
        let c = FxColors::of(palette_for_key("sum_flash"));
        assert!((c.primary_hdr - c.primary * 2.6).abs().max_element() < 1e-6);
        assert!((linear("#ffffff") - Vec3::ONE).abs().max_element() < 1e-6);
    }
}
