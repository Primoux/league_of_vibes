//! Texture de bruit tuilable partagée par les shaders du décor (sol, eau, falaises) : portage de
//! `packages/client/src/render/map/noise.ts`, mêmes valeurs à l'octet près.
//!
//! Un échantillon de texture coûte bien moins cher qu'un bruit calculé dans le shader : chaque canal porte
//! un fBm (bruit fractal) périodique de fréquence différente, combinés dans les shaders à plusieurs
//! échelles.
//!  - r : basse fréquence (taches, variations de teinte)
//!  - g : fréquence moyenne (bords organiques)
//!  - b : haute fréquence (grain)
//!  - a : bruit cellulaire (cailloux, pavés, caustiques)
//!
//! Le temps des shaders animés (`decorTime` du client web) est `globals.time` de Bevy.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

const N: usize = 256;

/// Entier JavaScript → `int32` (`x | 0`).
fn to_i32(x: i64) -> i32 {
    x as i32
}

pub(super) fn hash(i: i64, j: i64, seed: i64) -> f64 {
    let mut h = to_i32(i * 374_761_393 + j * 668_265_263 + seed * 2_147_483_647);
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1_274_126_177);
    f64::from((h ^ ((h as u32) >> 16) as i32) as u32) / 4_294_967_296.0
}

/// Modulo positif (`((i % p) + p) % p`).
fn wrap(i: i64, p: i64) -> i64 {
    i.rem_euclid(p)
}

/// Bruit de valeur périodique (période `p` cellules sur la texture).
fn value_noise(x: f64, y: f64, p: i64, seed: i64) -> f64 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let u = fx * fx * (3.0 - 2.0 * fx);
    let v = fy * fy * (3.0 - 2.0 * fy);
    let i0 = wrap(xi as i64, p);
    let j0 = wrap(yi as i64, p);
    let i1 = (i0 + 1) % p;
    let j1 = (j0 + 1) % p;
    let a = hash(i0, j0, seed);
    let b = hash(i1, j0, seed);
    let c = hash(i0, j1, seed);
    let d = hash(i1, j1, seed);
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

fn fbm(x: f64, y: f64, base: i64, octaves: i64, seed: i64) -> f64 {
    let (mut sum, mut amp, mut norm, mut p) = (0.0, 0.5, 0.0, base);
    let n = N as f64;
    for o in 0..octaves {
        sum += value_noise(x / n * p as f64, y / n * p as f64, p, seed + o * 17) * amp;
        norm += amp;
        amp *= 0.5;
        p *= 2;
    }
    sum / norm
}

/// Distance au point de Worley le plus proche (périodique), normalisée 0..1.
fn cellular(x: f64, y: f64, p: i64, seed: i64) -> f64 {
    let n = N as f64;
    let (cx, cy) = (x / n * p as f64, y / n * p as f64);
    let (xi, yi) = (cx.floor() as i64, cy.floor() as i64);
    let mut best: f64 = 9.0;
    for dj in -1..=1 {
        for di in -1..=1 {
            let (i, j) = (xi + di, yi + dj);
            let (ii, jj) = (wrap(i, p), wrap(j, p));
            let px = i as f64 + hash(ii, jj, seed);
            let py = j as f64 + hash(ii, jj, seed + 1);
            best = best.min((px - cx).hypot(py - cy));
        }
    }
    best.min(1.0)
}

/// Contraste étiré : les fBm se concentrent autour de 0,5, on les ramène sur toute la plage.
fn stretch(v: f64, k: f64) -> f64 {
    ((v - 0.5) * k + 0.5).clamp(0.0, 1.0)
}

/// `Uint8Array[k] = v * 255` : troncature.
fn byte(v: f64) -> u8 {
    (v * 255.0) as u8
}

/// Texels RGBA de la texture de bruit (ligne `y`, colonne `x`).
pub fn noise_data() -> Vec<u8> {
    let mut data = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let k = (y * N + x) * 4;
            let (fx, fy) = (x as f64, y as f64);
            data[k] = byte(stretch(fbm(fx, fy, 4, 4, 11), 1.9));
            data[k + 1] = byte(stretch(fbm(fx, fy, 8, 4, 23), 1.9));
            data[k + 2] = byte(stretch(fbm(fx, fy, 32, 3, 37), 1.7));
            data[k + 3] = byte(cellular(fx, fy, 24, 53));
        }
    }
    data
}

/// Image RGBA8 linéaire carrée avec sa chaîne de mipmaps (moyenne 2×2, comme `generateMipmaps`), filtrage
/// trilinéaire et anisotrope ×4. `repeat` : tuilable, sinon bord étiré (`ClampToEdgeWrapping`).
pub fn image_with_mips(size: usize, base: Vec<u8>, repeat: bool) -> Image {
    let extent = Extent3d {
        width: size as u32,
        height: size as u32,
        depth_or_array_layers: 1,
    };
    let mut data = base.clone();
    let (mut level, mut s) = (base, size);
    let mut levels = 1;
    while s > 1 {
        let h = s / 2;
        let mut next = vec![0u8; h * h * 4];
        for y in 0..h {
            for x in 0..h {
                for c in 0..4 {
                    let at = |xx: usize, yy: usize| u32::from(level[(yy * s + xx) * 4 + c]);
                    let sum = at(2 * x, 2 * y)
                        + at(2 * x + 1, 2 * y)
                        + at(2 * x, 2 * y + 1)
                        + at(2 * x + 1, 2 * y + 1);
                    next[(y * h + x) * 4 + c] = ((sum + 2) / 4) as u8;
                }
            }
        }
        data.extend_from_slice(&next);
        level = next;
        s = h;
        levels += 1;
    }
    let mut image = Image::new(
        extent,
        TextureDimension::D2,
        vec![0; size * size * 4],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    let address = if repeat {
        ImageAddressMode::Repeat
    } else {
        ImageAddressMode::ClampToEdge
    };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: address,
        address_mode_v: address,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 4,
        ..default()
    });
    image
}

/// Texture de bruit partagée par le décor, créée une fois au démarrage.
#[derive(Resource, Clone)]
pub struct NoiseTexture(pub Handle<Image>);

pub(super) fn create_noise_texture(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let handle = images.add(image_with_mips(N, noise_data(), true));
    commands.insert_resource(NoiseTexture(handle));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Texels de `noiseTexture().image.data` du client web (Node 24) : (x, y) → RGBA.
    #[test]
    fn memes_texels_que_le_typescript() {
        let data = noise_data();
        for (x, y, rgba) in NOISE_TEXELS {
            let k = (y * N + x) * 4;
            assert_eq!(&data[k..k + 4], &rgba, "texel ({x}, {y})");
        }
    }

    #[test]
    fn la_chaine_de_mipmaps_descend_jusqu_a_un_texel() {
        let image = image_with_mips(4, vec![255; 64], true);
        assert_eq!(image.texture_descriptor.mip_level_count, 3);
        assert_eq!(image.data.as_ref().map(Vec::len), Some((16 + 4 + 1) * 4));
    }

    const NOISE_TEXELS: [(usize, usize, [u8; 4]); 8] = [
        (0, 0, [149, 161, 221, 180]),
        (1, 0, [150, 160, 196, 179]),
        (17, 3, [149, 24, 173, 67]),
        (128, 128, [255, 162, 51, 63]),
        (255, 255, [151, 157, 221, 168]),
        (200, 13, [128, 101, 143, 67]),
        (77, 201, [58, 119, 160, 107]),
        (255, 0, [150, 159, 205, 158]),
    ];
}
