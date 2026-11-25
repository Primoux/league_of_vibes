//! Petit moteur de dessin vectoriel sur le processeur, pour les icônes du web (SVG en `viewBox`) : tracés
//! (`M L H V C A Z`, absolus) remplis en pair-impair avec anticrénelage, peintures unies et dégradés,
//! contour « sous le remplissage » (`paint-order: stroke`) par dilatation, flou, anneaux pointillés.
//!
//! Chaque icône est dessinée une fois dans une image, mise en cache ; Bevy l'affiche ensuite comme
//! n'importe quelle image. Les couleurs sont composées en sRGB, comme un navigateur.

use bevy::asset::RenderAssetUsages;
use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Contours fermés d'un tracé, en coordonnées de dessin.
#[derive(Clone, Debug, Default)]
pub struct Path {
    pub polys: Vec<Vec<Vec2>>,
}

/// Segments par courbe de Bézier et par quart d'arc.
const CURVE_STEPS: usize = 12;

fn numbers(s: &str) -> Vec<f32> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<f32>| {
        if !cur.is_empty() {
            if let Ok(v) = cur.parse() {
                out.push(v);
            }
            cur.clear();
        }
    };
    for c in s.chars() {
        match c {
            '0'..='9' | '.' => {
                // « 1.5.5 » : un second point commence un nouveau nombre
                if c == '.' && cur.contains('.') {
                    flush(&mut cur, &mut out);
                }
                cur.push(c);
            }
            '-' => {
                flush(&mut cur, &mut out);
                cur.push(c);
            }
            _ => flush(&mut cur, &mut out),
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// Arc elliptique SVG (`A rx ry rotation grand-arc sens x y`) depuis `p0`, en points (sans `p0`).
fn arc(p0: Vec2, rx: f32, ry: f32, rot_deg: f32, large: bool, sweep: bool, p1: Vec2) -> Vec<Vec2> {
    if p0 == p1 {
        return vec![];
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        return vec![p1];
    }
    // conversion vers la paramétrisation par le centre (SVG 1.1, annexe F.6.5)
    let phi = rot_deg.to_radians();
    let (sin, cos) = phi.sin_cos();
    let d = (p0 - p1) / 2.0;
    let x1 = Vec2::new(cos * d.x + sin * d.y, -sin * d.x + cos * d.y);
    let lambda = (x1.x / rx).powi(2) + (x1.y / ry).powi(2);
    if lambda > 1.0 {
        let k = lambda.sqrt();
        rx *= k;
        ry *= k;
    }
    let num = (rx * ry).powi(2) - (rx * x1.y).powi(2) - (ry * x1.x).powi(2);
    let den = (rx * x1.y).powi(2) + (ry * x1.x).powi(2);
    let mut coef = (num / den).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let c1 = Vec2::new(coef * rx * x1.y / ry, -coef * ry * x1.x / rx);
    let mid = (p0 + p1) / 2.0;
    let center = Vec2::new(cos * c1.x - sin * c1.y, sin * c1.x + cos * c1.y) + mid;
    let angle = |v: Vec2| v.y.atan2(v.x);
    let u = Vec2::new((x1.x - c1.x) / rx, (x1.y - c1.y) / ry);
    let v = Vec2::new((-x1.x - c1.x) / rx, (-x1.y - c1.y) / ry);
    let theta = angle(u);
    let mut delta = angle(v) - theta;
    let tau = std::f32::consts::TAU;
    if sweep && delta < 0.0 {
        delta += tau;
    } else if !sweep && delta > 0.0 {
        delta -= tau;
    }
    let steps = ((delta.abs() / (tau / 4.0)).ceil() as usize * CURVE_STEPS).max(2);
    (1..=steps)
        .map(|i| {
            let a = theta + delta * i as f32 / steps as f32;
            let p = Vec2::new(rx * a.cos(), ry * a.sin());
            Vec2::new(cos * p.x - sin * p.y, sin * p.x + cos * p.y) + center
        })
        .collect()
}

impl Path {
    /// Lit un attribut `d` (commandes absolues `M L H V C A Z`).
    pub fn parse(d: &str) -> Self {
        let mut polys: Vec<Vec<Vec2>> = Vec::new();
        let mut cur: Vec<Vec2> = Vec::new();
        let mut pos = Vec2::ZERO;
        let mut start = Vec2::ZERO;
        let mut i = 0;
        let bytes: Vec<char> = d.chars().collect();
        while i < bytes.len() {
            let cmd = bytes[i];
            if !cmd.is_ascii_alphabetic() {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < bytes.len() && !bytes[j].is_ascii_alphabetic() {
                j += 1;
            }
            let args = numbers(&bytes[i + 1..j].iter().collect::<String>());
            i = j;
            match cmd {
                'M' => {
                    if cur.len() > 1 {
                        polys.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    for (k, p) in args.as_chunks::<2>().0.iter().enumerate() {
                        pos = Vec2::new(p[0], p[1]);
                        if k == 0 {
                            start = pos;
                        }
                        cur.push(pos);
                    }
                }
                'L' => {
                    for p in args.as_chunks::<2>().0 {
                        pos = Vec2::new(p[0], p[1]);
                        cur.push(pos);
                    }
                }
                'H' => {
                    for x in &args {
                        pos.x = *x;
                        cur.push(pos);
                    }
                }
                'V' => {
                    for y in &args {
                        pos.y = *y;
                        cur.push(pos);
                    }
                }
                'C' => {
                    for c in args.as_chunks::<6>().0 {
                        let (p1, p2, p3) = (
                            Vec2::new(c[0], c[1]),
                            Vec2::new(c[2], c[3]),
                            Vec2::new(c[4], c[5]),
                        );
                        let p0 = pos;
                        for s in 1..=CURVE_STEPS {
                            let t = s as f32 / CURVE_STEPS as f32;
                            let u = 1.0 - t;
                            cur.push(
                                p0 * u * u * u
                                    + p1 * 3.0 * u * u * t
                                    + p2 * 3.0 * u * t * t
                                    + p3 * t * t * t,
                            );
                        }
                        pos = p3;
                    }
                }
                'A' => {
                    for a in args.as_chunks::<7>().0 {
                        let p1 = Vec2::new(a[5], a[6]);
                        cur.extend(arc(pos, a[0], a[1], a[2], a[3] != 0.0, a[4] != 0.0, p1));
                        pos = p1;
                    }
                }
                'Z' | 'z' => {
                    if cur.len() > 1 {
                        polys.push(std::mem::take(&mut cur));
                    }
                    pos = start;
                }
                _ => {}
            }
        }
        if cur.len() > 1 {
            polys.push(cur);
        }
        Self { polys }
    }

    /// Le tracé transformé (`transform` SVG).
    pub fn transformed(&self, t: Affine2) -> Self {
        Self {
            polys: self
                .polys
                .iter()
                .map(|p| p.iter().map(|v| t.transform_point2(*v)).collect())
                .collect(),
        }
    }
}

/// Couverture 0..1 de chaque pixel d'une image `w × h` par `path` (pair-impair), anticrénelée : quatre
/// lignes de balayage par pixel, recouvrement horizontal exact.
pub fn coverage(path: &Path, w: usize, h: usize) -> Vec<f32> {
    const SUB: usize = 4;
    let mut out = vec![0.0f32; w * h];
    let edges: Vec<(Vec2, Vec2)> = path
        .polys
        .iter()
        .flat_map(|p| {
            p.iter()
                .zip(p.iter().cycle().skip(1))
                .map(|(a, b)| (*a, *b))
        })
        .filter(|(a, b)| a.y != b.y)
        .collect();
    let mut xs: Vec<f32> = Vec::new();
    for row in 0..h {
        for s in 0..SUB {
            let y = row as f32 + (s as f32 + 0.5) / SUB as f32;
            xs.clear();
            for (a, b) in &edges {
                let (lo, hi) = if a.y < b.y { (a, b) } else { (b, a) };
                if y >= lo.y && y < hi.y {
                    xs.push(lo.x + (y - lo.y) / (hi.y - lo.y) * (hi.x - lo.x));
                }
            }
            xs.sort_by(f32::total_cmp);
            for span in xs.as_chunks::<2>().0 {
                let (x0, x1) = (span[0].max(0.0), span[1].min(w as f32));
                if x1 <= x0 {
                    continue;
                }
                let (first, last) = (x0.floor() as usize, (x1.ceil() as usize).min(w));
                for col in first..last {
                    let c0 = (col as f32).max(x0);
                    let c1 = (col as f32 + 1.0).min(x1);
                    if c1 > c0 {
                        out[row * w + col] += (c1 - c0) / SUB as f32;
                    }
                }
            }
        }
    }
    for v in &mut out {
        *v = v.min(1.0);
    }
    out
}

/// Couverture élargie de `r` pixels (contour de largeur 2r centré sur le bord, vu sous le remplissage).
pub fn dilate(mask: &[f32], w: usize, h: usize, r: f32) -> Vec<f32> {
    let n = r.ceil() as i32;
    let mut out = vec![0.0f32; mask.len()];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut m = 0.0f32;
            for dy in -n..=n {
                for dx in -n..=n {
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    let (sx, sy) = (x + dx, y + dy);
                    if d > r + 0.5 || sx < 0 || sy < 0 || sx >= w as i32 || sy >= h as i32 {
                        continue;
                    }
                    let k = (r + 0.5 - d).clamp(0.0, 1.0);
                    m = m.max(mask[sy as usize * w + sx as usize] * k);
                }
            }
            out[y as usize * w + x as usize] = m;
        }
    }
    out
}

/// Flou gaussien approché (trois flous en boîte) d'écart-type `sigma` pixels.
pub fn blur(mask: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let r = ((sigma * sigma * 4.0 + 1.0).sqrt() / 2.0).round().max(1.0) as i32;
    let mut a = mask.to_vec();
    let mut b = vec![0.0f32; mask.len()];
    for _ in 0..3 {
        box_pass(&a, &mut b, w, h, r, true);
        box_pass(&b, &mut a, w, h, r, false);
    }
    a
}

fn box_pass(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: i32, horizontal: bool) {
    let n = (2 * r + 1) as f32;
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut s = 0.0;
            for k in -r..=r {
                let (sx, sy) = if horizontal { (x + k, y) } else { (x, y + k) };
                if sx >= 0 && sy >= 0 && sx < w as i32 && sy < h as i32 {
                    s += src[sy as usize * w + sx as usize];
                }
            }
            dst[y as usize * w + x as usize] = s / n;
        }
    }
}

/// Image RGBA en sRGB, alpha prémultiplié pendant le dessin.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    /// Pixels prémultipliés.
    px: Vec<[f32; 4]>,
    /// Pixels par unité du `viewBox`.
    pub scale: f32,
}

/// Couleur sRGB 0..1 non prémultipliée.
pub type Rgba = [f32; 4];

/// `#rrggbb` → couleur, avec opacité.
pub fn rgb(c: u32, a: f32) -> Rgba {
    [
        ((c >> 16) & 255) as f32 / 255.0,
        ((c >> 8) & 255) as f32 / 255.0,
        (c & 255) as f32 / 255.0,
        a,
    ]
}

/// Couleur CSS `#rrggbb` (blanc si illisible).
pub fn css(s: &str, a: f32) -> Rgba {
    u32::from_str_radix(s.trim_start_matches('#'), 16).map_or([1.0, 1.0, 1.0, a], |c| rgb(c, a))
}

/// Mélange linéaire de couleurs aux arrêts (`offset`, couleur) triés, en `t`.
pub fn stops(list: &[(f32, Rgba)], t: f32) -> Rgba {
    let Some(first) = list.first() else {
        return [0.0; 4];
    };
    if t <= first.0 {
        return first.1;
    }
    for w in list.windows(2) {
        let ((o0, c0), (o1, c1)) = (w[0], w[1]);
        if t <= o1 {
            let k = if o1 > o0 { (t - o0) / (o1 - o0) } else { 1.0 };
            return std::array::from_fn(|i| c0[i] + (c1[i] - c0[i]) * k);
        }
    }
    list[list.len() - 1].1
}

impl Canvas {
    /// Toile de `size` pixels pour un `viewBox` carré de `view` unités.
    pub fn new(size: usize, view: f32) -> Self {
        Self {
            w: size,
            h: size,
            px: vec![[0.0; 4]; size * size],
            scale: size as f32 / view,
        }
    }

    /// Transformation `viewBox` → pixels, après `t` (repère de l'icône).
    pub fn to_px(&self, t: Affine2) -> Affine2 {
        Affine2::from_scale(Vec2::splat(self.scale)) * t
    }

    /// Couverture d'un tracé exprimé dans le `viewBox` (transformé par `t`).
    pub fn mask(&self, path: &Path, t: Affine2) -> Vec<f32> {
        coverage(&path.transformed(self.to_px(t)), self.w, self.h)
    }

    /// Peint `paint` (fonction du point, en unités du `viewBox`) à travers `mask`, avec `opacity`.
    pub fn paint(&mut self, mask: &[f32], opacity: f32, paint: impl Fn(Vec2) -> Rgba) {
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y * self.w + x;
                let m = mask[i] * opacity;
                if m <= 0.0 {
                    continue;
                }
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / self.scale;
                let c = paint(p);
                let a = c[3] * m;
                let d = &mut self.px[i];
                for k in 0..3 {
                    d[k] = c[k] * a + d[k] * (1.0 - a);
                }
                d[3] = a + d[3] * (1.0 - a);
            }
        }
    }

    /// Masque plein.
    pub fn full(&self) -> Vec<f32> {
        vec![1.0; self.w * self.h]
    }

    /// Anneau de rayon `r` et d'épaisseur `width` centré en `c` (unités du `viewBox`), en pointillés
    /// `dash` (longueur tracée, longueur vide) s'il y en a.
    pub fn ring(&self, c: Vec2, r: f32, width: f32, dash: Option<(f32, f32)>) -> Vec<f32> {
        let mut out = vec![0.0; self.w * self.h];
        let half = width * self.scale / 2.0;
        for y in 0..self.h {
            for x in 0..self.w {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / self.scale - c;
                let d = (p.length() - r).abs() * self.scale;
                let mut m = (half + 0.5 - d).clamp(0.0, 1.0).min(width * self.scale);
                if let Some((on, off)) = dash {
                    // le cercle SVG part de (cx + r, cy) dans le sens des aiguilles d'une montre
                    let a = p.y.atan2(p.x).rem_euclid(std::f32::consts::TAU);
                    if (a * r).rem_euclid(on + off) > on {
                        m = 0.0;
                    }
                }
                out[y * self.w + x] = m;
            }
        }
        out
    }

    /// Image Bevy (sRGB, alpha droit).
    pub fn into_image(self) -> Image {
        let mut data = Vec::with_capacity(self.px.len() * 4);
        for p in &self.px {
            let a = p[3];
            let un = |v: f32| {
                if a > 0.0 {
                    (v / a).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            };
            data.extend([
                (un(p[0]) * 255.0).round() as u8,
                (un(p[1]) * 255.0).round() as u8,
                (un(p[2]) * 255.0).round() as u8,
                (a.clamp(0.0, 1.0) * 255.0).round() as u8,
            ]);
        }
        Image::new(
            Extent3d {
                width: self.w as u32,
                height: self.h as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        )
    }

    /// Pixel (alpha droit), pour les tests.
    #[cfg(test)]
    pub fn pixel(&self, x: usize, y: usize) -> Rgba {
        let p = self.px[y * self.w + x];
        if p[3] > 0.0 {
            [p[0] / p[3], p[1] / p[3], p[2] / p[3], p[3]]
        } else {
            [0.0; 4]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(mask: &[f32]) -> f32 {
        mask.iter().sum()
    }

    #[test]
    fn carre_et_trou_pair_impair() {
        // carré 10 × 10 percé d'un carré 4 × 4 (même sens de parcours : le pair-impair le perce)
        let p = Path::parse("M2 2 H12 V12 H2 Z M5 5 H9 V9 H5 Z");
        let m = coverage(&p, 16, 16);
        assert!((area(&m) - 84.0).abs() < 0.01, "{}", area(&m));
        assert_eq!(m[6 * 16 + 6], 0.0);
        assert_eq!(m[3 * 16 + 3], 1.0);
    }

    #[test]
    fn cercle_par_deux_arcs_presque_fermes() {
        // le motif des glyphes : un cercle tracé par un arc de 359,9°
        let p = Path::parse("M12 2 A10 10 0 1 1 11.99 2 Z");
        let m = coverage(&p, 24, 24);
        let expect = std::f32::consts::PI * 100.0;
        assert!((area(&m) - expect).abs() < expect * 0.02, "{}", area(&m));
    }

    #[test]
    fn courbe_de_bezier_et_nombres_colles() {
        let p = Path::parse("M2 12 C2 6 22 6 22 12 Z");
        assert_eq!(p.polys.len(), 1);
        assert_eq!(p.polys[0].len(), 1 + CURVE_STEPS);
        assert_eq!(numbers("1.5.5-2"), vec![1.5, 0.5, -2.0]);
    }

    #[test]
    fn dilatation_et_flou_conservent_le_centre() {
        let p = Path::parse("M6 6 H10 V10 H6 Z");
        let m = coverage(&p, 16, 16);
        let d = dilate(&m, 16, 16, 1.0);
        assert!(area(&d) > area(&m));
        // un pixel au-dessus du bord : à moitié couvert par un contour de rayon 1
        assert!((d[5 * 16 + 8] - 0.5).abs() < 1e-5);
        let b = blur(&m, 16, 16, 1.6);
        assert!((area(&b) - area(&m)).abs() < 1.0);
        assert!(b[8 * 16 + 8] < 1.0 && b[8 * 16 + 8] > 0.3);
    }

    #[test]
    fn peinture_et_degrades() {
        let mut c = Canvas::new(8, 8.0);
        let full = c.full();
        c.paint(&full, 1.0, |_| rgb(0xff0000, 1.0));
        c.paint(&full, 0.5, |_| rgb(0x0000ff, 1.0));
        let px = c.pixel(0, 0);
        assert!((px[0] - 0.5).abs() < 1e-5 && (px[2] - 0.5).abs() < 1e-5 && px[3] == 1.0);
        let g = [(0.0, rgb(0x000000, 1.0)), (1.0, rgb(0xffffff, 1.0))];
        assert_eq!(stops(&g, 0.5)[0], 0.5);
        assert_eq!(stops(&g, 2.0), g[1].1);
        let ring = c.ring(Vec2::splat(4.0), 3.0, 1.0, Some((2.0, 3.0)));
        assert!(ring.iter().any(|v| *v > 0.0) && ring.contains(&0.0));
    }
}
