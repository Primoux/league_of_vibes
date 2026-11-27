//! Icônes et portraits : portage de `packages/client/src/ui/icons/` (area `ui-core`).
//!
//! Portraits de champions (`ChampionPortrait`) et illustrations peintes (`ChampionArtwork.tsx`) ; tracés
//! des icônes (`paths`), choix des glyphes (`glyphs`) et leur dessin (`raster`).

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;

// Glyphes et dessin servent aux icônes du HUD (#113), branchées dans la PR suivante : en attendant,
// seuls les tests les lisent.
#[allow(dead_code)]
pub mod glyphs;
#[allow(dead_code)]
pub mod paths;
pub mod portrait;
#[allow(dead_code)]
pub mod raster;

pub use portrait::{ChampionPortrait, PortraitKind};

/// Illustrations peintes des champions (WebP 512 × 512), embarquées dans l'exécutable.
const ARTWORKS: [(&str, &[u8]); 13] = [
    ("corvin", include_bytes!("champions/corvin.webp")),
    ("grondak", include_bytes!("champions/grondak.webp")),
    ("ilyra", include_bytes!("champions/ilyra.webp")),
    ("kaelen", include_bytes!("champions/kaelen.webp")),
    ("master67", include_bytes!("champions/master67.webp")),
    ("melenchon", include_bytes!("champions/melenchon.webp")),
    ("nyx", include_bytes!("champions/nyx.webp")),
    ("sable", include_bytes!("champions/sable.webp")),
    ("solenne", include_bytes!("champions/solenne.webp")),
    ("tharox", include_bytes!("champions/tharox.webp")),
    ("touilleur", include_bytes!("champions/touilleur.webp")),
    ("vrenna", include_bytes!("champions/vrenna.webp")),
    ("zephyr", include_bytes!("champions/zephyr.webp")),
];

/// Décode une illustration.
pub fn decode(bytes: &[u8]) -> Option<Image> {
    Image::from_buffer(
        bytes,
        ImageType::Extension("webp"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::linear(),
        RenderAssetUsages::RENDER_WORLD,
    )
    .ok()
}

/// Copie fondue sur les bords (`mask: radial-gradient(closest-side, #000 45%, transparent)`) : illustration
/// d'ambiance derrière la scène 3D.
pub fn faded(image: &Image) -> Option<Image> {
    let (w, h) = (image.width(), image.height());
    let mut out = image
        .clone()
        .convert(bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb)?;
    let data = out.data.as_mut()?;
    let half = w.min(h) as f32 / 2.0;
    for y in 0..h {
        for x in 0..w {
            let d = Vec2::new(
                x as f32 + 0.5 - w as f32 / 2.0,
                y as f32 + 0.5 - h as f32 / 2.0,
            )
            .length()
                / half;
            let k = (1.0 - ((d - 0.45) / 0.55).clamp(0.0, 1.0)).clamp(0.0, 1.0);
            let i = ((y * w + x) * 4 + 3) as usize;
            data[i] = (f32::from(data[i]) * k).round() as u8;
        }
    }
    Some(out)
}

/// Illustrations prêtes, par champion : nette (portraits) et fondue (ambiance).
#[derive(Resource, Default)]
pub struct Artworks {
    sharp: HashMap<&'static str, Handle<Image>>,
    faded: HashMap<&'static str, Handle<Image>>,
}

impl Artworks {
    /// Illustration du champion, s'il en a une (`championArtworkUrl`).
    pub fn get(&self, id: &str) -> Option<Handle<Image>> {
        self.sharp.get(id).cloned()
    }

    /// Illustration fondue sur les bords, pour le fond de la scène.
    pub fn faded(&self, id: &str) -> Option<Handle<Image>> {
        self.faded.get(id).cloned()
    }
}

pub struct IconsPlugin;

impl Plugin for IconsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(portrait::PortraitPlugin);
    }

    /// Illustrations décodées avant le premier écran (l'accueil les affiche dès le démarrage).
    fn finish(&self, app: &mut App) {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        let mut art = Artworks::default();
        for (id, bytes) in ARTWORKS {
            let Some(image) = decode(bytes) else {
                warn!("illustration de {id} illisible : portrait de secours");
                continue;
            };
            if let Some(f) = faded(&image) {
                art.faded.insert(id, images.add(f));
            }
            art.sharp.insert(id, images.add(image));
        }
        app.insert_resource(art);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::data::champions::CHAMPIONS;

    #[test]
    fn une_illustration_lisible_par_champion() {
        for c in CHAMPIONS.iter() {
            let Some((_, bytes)) = ARTWORKS.iter().find(|(id, _)| *id == c.id) else {
                panic!("pas d'illustration pour {}", c.id);
            };
            let Some(img) = decode(bytes) else {
                panic!("illustration de {} illisible", c.id);
            };
            assert_eq!((img.width(), img.height()), (512, 512), "{}", c.id);
        }
    }

    #[test]
    fn illustration_fondue_sur_les_bords() {
        let Some(img) = decode(ARTWORKS[0].1) else {
            panic!("illisible");
        };
        let Some(f) = faded(&img) else {
            panic!("conversion impossible");
        };
        let data = f.data.unwrap_or_default();
        let alpha = |x: usize, y: usize| data[(y * 512 + x) * 4 + 3];
        assert_eq!(alpha(256, 256), 255);
        assert!(alpha(0, 256) <= 1);
        assert_eq!(alpha(2, 2), 0);
    }
}
