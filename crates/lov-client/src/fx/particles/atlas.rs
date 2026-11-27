//! Atlas de textures de particules dessinées procéduralement (aucun asset externe).
//! Grille 4×4 de cellules de 128 px, masques blancs (la couleur vient du style et de la teinte).
//!
//! Portage de `packages/client/src/fx/particles/textures.ts`. Le client web dessine dans un canvas 2D ; ici un
//! petit rastériseur logiciel reproduit les mêmes formes (dégradés, polygones, traits, ombres floues), avec
//! un suréchantillonnage 4×4 pour l'anticrénelage. Seul l'alpha compte : le rgb de l'atlas est blanc partout.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::math::{Mat2, Vec2};

/// Indices des cellules (`TEX` du client web).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tex {
    Glow = 0,
    Spark = 1,
    Smoke = 2,
    Ring = 3,
    Star = 4,
    Snowflake = 5,
    Feather = 6,
    Rune = 7,
    Shard = 8,
    Dot = 9,
    Streak = 10,
    Cross = 11,
    Coin = 12,
    Flame = 13,
    Swirl = 14,
    Sparkles = 15,
}

impl Tex {
    pub const ALL: [Tex; 16] = [
        Tex::Glow,
        Tex::Spark,
        Tex::Smoke,
        Tex::Ring,
        Tex::Star,
        Tex::Snowflake,
        Tex::Feather,
        Tex::Rune,
        Tex::Shard,
        Tex::Dot,
        Tex::Streak,
        Tex::Cross,
        Tex::Coin,
        Tex::Flame,
        Tex::Swirl,
        Tex::Sparkles,
    ];
}

pub const CELL: usize = 128;
pub const GRID: usize = 4;
pub const SIZE: usize = CELL * GRID;
const S: f32 = CELL as f32;
/// Suréchantillonnage par axe.
const SS: usize = 4;

/// Bruit déterministe pour que l'atlas soit identique à chaque lancement (même formule que le client web).
fn hash(n: f64) -> f32 {
    let x = (n * 127.1).sin() * 43758.5453;
    (x - x.floor()) as f32
}

/// Modes de composition du canvas utilisés par les dessins.
#[derive(Clone, Copy)]
enum Op {
    /// `source-over`
    Over,
    /// `destination-out`
    Out,
    /// `destination-in` (appliqué à toute la cellule)
    In,
}

/// Une cellule : alpha `CELL × CELL`, ligne 0 en haut (comme le canvas).
struct Cell {
    a: Vec<f32>,
}

impl Cell {
    fn new() -> Self {
        Self {
            a: vec![0.0; CELL * CELL],
        }
    }

    /// Peint `paint(p)` (alpha source au point `p`, repère local) à travers la transformation `xf`.
    /// `xf` envoie le repère local vers les pixels de la cellule : `pixel = origin + mat · local`.
    fn paint(&mut self, xf: Xf, op: Op, paint: impl Fn(Vec2) -> f32) {
        let mask = rasterize(xf, &paint);
        self.composite(&mask, op);
    }

    /// Comme `paint`, avec l'ombre floue du canvas (`shadowBlur`, couleur blanche) dessinée dessous.
    fn paint_shadowed(&mut self, xf: Xf, blur: f32, paint: impl Fn(Vec2) -> f32) {
        let mask = rasterize(xf, &paint);
        let shadow = gaussian_blur(&mask, blur / 2.0);
        self.composite(&shadow, Op::Over);
        self.composite(&mask, Op::Over);
    }

    fn composite(&mut self, src: &[f32], op: Op) {
        for (d, &s) in self.a.iter_mut().zip(src) {
            *d = match op {
                Op::Over => s + *d * (1.0 - s),
                Op::Out => *d * (1.0 - s),
                Op::In => *d * s,
            };
        }
    }
}

/// Transformation affine local → pixels de la cellule (`translate` / `rotate` du canvas).
#[derive(Clone, Copy)]
struct Xf {
    origin: Vec2,
    mat: Mat2,
}

impl Xf {
    const ID: Xf = Xf {
        origin: Vec2::ZERO,
        mat: Mat2::IDENTITY,
    };

    fn translate(x: f32, y: f32) -> Xf {
        Xf {
            origin: Vec2::new(x, y),
            mat: Mat2::IDENTITY,
        }
    }

    /// Rotation du canvas (y vers le bas : un angle positif tourne dans le sens horaire à l'écran).
    fn rotate(self, a: f32) -> Xf {
        Xf {
            origin: self.origin,
            mat: self.mat * Mat2::from_angle(a),
        }
    }

    fn inverse(self, px: Vec2) -> Vec2 {
        self.mat.inverse() * (px - self.origin)
    }
}

fn rasterize(xf: Xf, paint: &impl Fn(Vec2) -> f32) -> Vec<f32> {
    let mut out = vec![0.0; CELL * CELL];
    let step = 1.0 / SS as f32;
    for y in 0..CELL {
        for x in 0..CELL {
            let mut sum = 0.0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let px = Vec2::new(
                        x as f32 + (sx as f32 + 0.5) * step,
                        y as f32 + (sy as f32 + 0.5) * step,
                    );
                    sum += paint(xf.inverse(px)).clamp(0.0, 1.0);
                }
            }
            out[y * CELL + x] = sum / (SS * SS) as f32;
        }
    }
    out
}

/// Flou gaussien séparable (bords transparents), écart-type `sigma` en pixels.
fn gaussian_blur(src: &[f32], sigma: f32) -> Vec<f32> {
    let r = (sigma * 3.0).ceil() as i32;
    let kernel: Vec<f32> = (-r..=r)
        .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    let norm: f32 = kernel.iter().sum();
    let pass = |src: &[f32], horizontal: bool| {
        let mut out = vec![0.0; CELL * CELL];
        for y in 0..CELL as i32 {
            for x in 0..CELL as i32 {
                let mut acc = 0.0;
                for (k, w) in kernel.iter().enumerate() {
                    let o = k as i32 - r;
                    let (sx, sy) = if horizontal { (x + o, y) } else { (x, y + o) };
                    if (0..CELL as i32).contains(&sx) && (0..CELL as i32).contains(&sy) {
                        acc += w * src[(sy * CELL as i32 + sx) as usize];
                    }
                }
                out[(y * CELL as i32 + x) as usize] = acc / norm;
            }
        }
        out
    };
    pass(&pass(src, true), false)
}

// ------------------------------------------------------------------------------------------------
// Primitives de dessin (repère local, en pixels)
// ------------------------------------------------------------------------------------------------

/// Dégradé linéaire par morceaux (`addColorStop`), constant hors des bornes.
fn stops(st: &[[f32; 2]], t: f32) -> f32 {
    crate::fx::core::curves::sample_curve(st, t)
}

/// Point dans un polygone (règle non nulle, celle du canvas).
fn in_poly(p: Vec2, pts: &[Vec2]) -> bool {
    let mut winding = 0;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        let side = (b - a).perp_dot(p - a);
        if a.y <= p.y {
            if b.y > p.y && side > 0.0 {
                winding += 1;
            }
        } else if b.y <= p.y && side < 0.0 {
            winding -= 1;
        }
    }
    winding != 0
}

fn in_ellipse(p: Vec2, rx: f32, ry: f32) -> bool {
    (p.x / rx).powi(2) + (p.y / ry).powi(2) <= 1.0
}

/// Distance d'un point à un segment ; `butt` : extrémités coupées net (sinon arrondies).
fn seg_dist(p: Vec2, a: Vec2, b: Vec2, butt: bool) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 {
        (p - a).dot(ab) / len2
    } else {
        0.0
    };
    if butt && !(0.0..=1.0).contains(&t) {
        return f32::INFINITY;
    }
    (p - (a + ab * t.clamp(0.0, 1.0))).length()
}

/// Trait d'une suite de segments, largeur `w`.
fn stroke(p: Vec2, segs: &[(Vec2, Vec2)], w: f32, butt: bool) -> f32 {
    let hit = segs
        .iter()
        .any(|&(a, b)| seg_dist(p, a, b, butt) <= w / 2.0);
    if hit { 1.0 } else { 0.0 }
}

fn bool_alpha(b: bool) -> f32 {
    if b { 1.0 } else { 0.0 }
}

/// Fond d'un dégradé radial centré dans la cellule (`radial` du client web).
fn radial(cell: &mut Cell, st: &[[f32; 2]]) {
    cell.paint(Xf::ID, Op::Over, |p| {
        stops(st, (p - Vec2::splat(S / 2.0)).length() / (S / 2.0))
    });
}

/// Courbe de Bézier cubique échantillonnée.
fn bezier(out: &mut Vec<Vec2>, p0: Vec2, c1: Vec2, c2: Vec2, p1: Vec2) {
    const N: usize = 32;
    for i in 1..=N {
        let t = i as f32 / N as f32;
        let u = 1.0 - t;
        out.push(p0 * u * u * u + c1 * 3.0 * u * u * t + c2 * 3.0 * u * t * t + p1 * t * t * t);
    }
}

// ------------------------------------------------------------------------------------------------
// Dessins des cellules (`PAINTERS` du client web)
// ------------------------------------------------------------------------------------------------

fn paint_cell(tex: Tex, c: &mut Cell) {
    let center = Xf::translate(S / 2.0, S / 2.0);
    let v = Vec2::new;
    match tex {
        Tex::Glow => radial(c, &[[0.0, 1.0], [0.2, 0.75], [0.5, 0.22], [1.0, 0.0]]),
        Tex::Spark => {
            radial(c, &[[0.0, 1.0], [0.12, 0.5], [0.35, 0.0], [1.0, 0.0]]);
            let diamond = [
                v(-S / 2.0, 0.0),
                v(0.0, -S * 0.035),
                v(S / 2.0, 0.0),
                v(0.0, S * 0.035),
            ];
            for i in 0..2 {
                c.paint(center.rotate(i as f32 * FRAC_PI_2), Op::Over, |p| {
                    let g = stops(&[[0.0, 0.0], [0.5, 1.0], [1.0, 0.0]], (p.x + S / 2.0) / S);
                    g * bool_alpha(in_poly(p, &diamond))
                });
            }
        }
        Tex::Smoke => {
            for i in 0..14 {
                let r = S * (0.12 + hash(f64::from(i)) * 0.18);
                let o = v(
                    S / 2.0 + (hash(f64::from(i + 20)) - 0.5) * S * 0.38,
                    S / 2.0 + (hash(f64::from(i + 40)) - 0.5) * S * 0.38,
                );
                c.paint(Xf::ID, Op::Over, |p| {
                    stops(&[[0.0, 0.35], [1.0, 0.0]], (p - o).length() / r)
                });
            }
        }
        Tex::Ring => radial(
            c,
            &[[0.0, 0.0], [0.62, 0.0], [0.8, 1.0], [0.9, 0.4], [1.0, 0.0]],
        ),
        Tex::Star => {
            let pts: Vec<Vec2> = (0..10)
                .map(|i| {
                    let r = if i % 2 == 1 { 0.17 } else { 0.4 } * S;
                    let a = (i as f32 / 10.0) * TAU - FRAC_PI_2;
                    v(a.cos() * r, a.sin() * r)
                })
                .collect();
            c.paint_shadowed(center, S * 0.08, |p| bool_alpha(in_poly(p, &pts)));
        }
        Tex::Snowflake => {
            let mut segs = Vec::new();
            for i in 0..6 {
                let m = Mat2::from_angle(i as f32 * PI / 3.0);
                let r = |x: f32, y: f32| m * v(x, y);
                segs.push((r(0.0, 0.0), r(0.0, -S * 0.4)));
                segs.push((r(0.0, -S * 0.24), r(-S * 0.09, -S * 0.33)));
                segs.push((r(0.0, -S * 0.24), r(S * 0.09, -S * 0.33)));
            }
            c.paint_shadowed(center, S * 0.05, |p| stroke(p, &segs, S * 0.045, false));
        }
        Tex::Feather => {
            let xf = center.rotate(-PI / 4.0);
            c.paint(xf, Op::Over, |p| {
                bool_alpha(in_ellipse(p, S * 0.13, S * 0.42))
            });
            let segs: Vec<(Vec2, Vec2)> = (-6..=6)
                .map(|i| {
                    let y = i as f32 * S * 0.05;
                    let side = if i == 0 { 1.0 } else { (i as f32).signum() };
                    (v(0.0, y), v(S * 0.15 * side, y - S * 0.06))
                })
                .collect();
            c.paint(xf, Op::Out, |p| stroke(p, &segs, S * 0.012, true));
            c.paint(xf, Op::Out, |p| {
                bool_alpha(p.x.abs() <= S * 0.008 && p.y.abs() <= S * 0.42)
            });
        }
        Tex::Rune => {
            let tri: Vec<(Vec2, Vec2)> = (0..3)
                .map(|i| {
                    let a = (i as f32 / 3.0) * TAU - FRAC_PI_2;
                    let b = a + TAU / 3.0;
                    (
                        v(a.cos(), a.sin()) * S * 0.33,
                        v(b.cos(), b.sin()) * S * 0.33,
                    )
                })
                .collect();
            let dots: Vec<Vec2> = (0..12)
                .map(|i| {
                    let a = (i as f32 / 12.0) * TAU;
                    v(a.cos(), a.sin()) * S * 0.375
                })
                .collect();
            c.paint_shadowed(center, S * 0.04, |p| {
                let r = p.length();
                let outer = (r - S * 0.42).abs() <= S * 0.015;
                let inner = (r - S * 0.33).abs() <= S * 0.009;
                let lines = stroke(p, &tri, S * 0.018, true) > 0.0;
                let dot = dots
                    .iter()
                    .any(|d| (p.x - d.x).abs() <= 2.0 && (p.y - d.y).abs() <= 2.0);
                bool_alpha(outer || inner || lines || dot)
            });
        }
        Tex::Shard => {
            let pts = [
                v(0.0, -S * 0.45),
                v(S * 0.16, 0.0),
                v(0.0, S * 0.45),
                v(-S * 0.16, 0.0),
            ];
            c.paint(center, Op::Over, |p| {
                let g = stops(
                    &[[0.0, 0.55], [0.5, 1.0], [1.0, 0.4]],
                    (p.x + S * 0.15) / (S * 0.3),
                );
                g * bool_alpha(in_poly(p, &pts))
            });
        }
        Tex::Dot => radial(c, &[[0.0, 1.0], [0.55, 1.0], [0.75, 0.35], [1.0, 0.0]]),
        Tex::Streak => {
            c.paint(Xf::ID, Op::Over, |p| {
                let g = stops(&[[0.0, 0.0], [0.7, 1.0], [1.0, 0.0]], p.x / S);
                g * bool_alpha(p.y >= S * 0.42 && p.y <= S * 0.58)
            });
            c.paint(Xf::ID, Op::In, |p| {
                stops(
                    &[[0.0, 0.0], [0.5, 1.0], [1.0, 0.0]],
                    (p.y - S * 0.35) / (S * 0.3),
                )
            });
        }
        Tex::Cross => {
            let (w, l) = (S * 0.12, S * 0.34);
            c.paint_shadowed(center, S * 0.08, |p| {
                let (x, y) = (p.x.abs(), p.y.abs());
                bool_alpha((x <= w && y <= l) || (x <= l && y <= w))
            });
        }
        Tex::Coin => {
            c.paint(center, Op::Over, |p| {
                bool_alpha(in_ellipse(p, S * 0.36, S * 0.4))
            });
            // trait d'ellipse : approché par la différence de deux ellipses (demi-largeur 0,02·S)
            let h = S * 0.02;
            c.paint(center, Op::Out, |p| {
                bool_alpha(
                    in_ellipse(p, S * 0.26 + h, S * 0.3 + h)
                        && !in_ellipse(p, S * 0.26 - h, S * 0.3 - h),
                )
            });
        }
        Tex::Flame => {
            let mut path = vec![v(0.0, -S * 0.55)];
            bezier(
                &mut path,
                v(0.0, -S * 0.55),
                v(S * 0.3, -S * 0.2),
                v(S * 0.32, S * 0.1),
                v(0.0, S * 0.3),
            );
            bezier(
                &mut path,
                v(0.0, S * 0.3),
                v(-S * 0.32, S * 0.1),
                v(-S * 0.3, -S * 0.2),
                v(0.0, -S * 0.55),
            );
            c.paint(Xf::translate(S / 2.0, S * 0.62), Op::Over, |p| {
                let g = stops(
                    &[[0.0, 1.0], [0.6, 0.6], [1.0, 0.0]],
                    p.length() / (S * 0.36),
                );
                g * bool_alpha(in_poly(p, &path))
            });
        }
        Tex::Swirl => {
            // Le client web règle `lineWidth` dans la boucle mais ne trace qu'après : c'est la dernière valeur
            // (1 px) qui s'applique à tout le bras. On garde ce rendu fin.
            let mut segs = Vec::new();
            for k in 0..3 {
                let m = Mat2::from_angle((k + 1) as f32 * TAU / 3.0);
                let pts: Vec<Vec2> = (0..=40)
                    .map(|i| {
                        let t = i as f32 / 40.0;
                        let a = t * PI * 1.4;
                        m * (v(a.cos(), a.sin()) * S * (0.06 + 0.36 * t))
                    })
                    .collect();
                segs.extend(pts.windows(2).map(|w| (w[0], w[1])));
            }
            c.paint(center, Op::Over, |p| stroke(p, &segs, 1.0, false));
        }
        Tex::Sparkles => {
            for i in 0..9 {
                let o = v(
                    S * (0.15 + hash(f64::from(i + 3)) * 0.7),
                    S * (0.15 + hash(f64::from(i + 11)) * 0.7),
                );
                let r = S * (0.03 + hash(f64::from(i + 17)) * 0.05);
                c.paint(Xf::ID, Op::Over, |p| {
                    stops(&[[0.0, 1.0], [1.0, 0.0]], (p - o).length() / r)
                });
            }
        }
    }
}

/// Une seule cellule en RGBA8 (`CELL × CELL`) : rgb blanc, alpha = masque (`atlasCell` du client web,
/// pour les sprites du runtime FX).
pub fn cell_rgba(tex: Tex) -> Vec<u8> {
    let mut cell = Cell::new();
    paint_cell(tex, &mut cell);
    cell.a
        .iter()
        .flat_map(|a| [255, 255, 255, (a.clamp(0.0, 1.0) * 255.0).round() as u8])
        .collect()
}

/// Atlas RGBA8 (`SIZE × SIZE`, ligne 0 en haut) : rgb blanc, alpha = masque de la cellule.
pub fn build_atlas() -> Vec<u8> {
    let mut rgba = vec![255u8; SIZE * SIZE * 4];
    for tex in Tex::ALL {
        let mut cell = Cell::new();
        paint_cell(tex, &mut cell);
        let idx = tex as usize;
        let (cx, cy) = ((idx % GRID) * CELL, (idx / GRID) * CELL);
        for y in 0..CELL {
            for x in 0..CELL {
                let a = (cell.a[y * CELL + x].clamp(0.0, 1.0) * 255.0).round() as u8;
                rgba[((cy + y) * SIZE + cx + x) * 4 + 3] = a;
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(atlas: &[u8], tex: Tex, x: usize, y: usize) -> u8 {
        let idx = tex as usize;
        let (cx, cy) = ((idx % GRID) * CELL, (idx / GRID) * CELL);
        atlas[((cy + y) * SIZE + cx + x) * 4 + 3]
    }

    #[test]
    fn chaque_cellule_est_dessinee_et_s_annule_dans_les_coins() {
        let atlas = build_atlas();
        for tex in Tex::ALL {
            let idx = tex as usize;
            let (cx, cy) = ((idx % GRID) * CELL, (idx / GRID) * CELL);
            let mut sum = 0u64;
            for y in 0..CELL {
                for x in 0..CELL {
                    sum += u64::from(atlas[((cy + y) * SIZE + cx + x) * 4 + 3]);
                }
            }
            assert!(sum > 0, "cellule {tex:?} vide");
            assert_eq!(
                alpha(&atlas, tex, 0, 0),
                0,
                "coin de {tex:?} non transparent"
            );
        }
    }

    #[test]
    fn formes_reperes() {
        let atlas = build_atlas();
        let mid = CELL / 2;
        assert!(alpha(&atlas, Tex::Glow, mid, mid) > 240);
        // anneau : creux au centre, plein vers 80 % du rayon
        assert_eq!(alpha(&atlas, Tex::Ring, mid, mid), 0);
        assert!(alpha(&atlas, Tex::Ring, mid + (0.8 * 64.0) as usize, mid) > 200);
        // traînée : tête lumineuse vers 70 % de la largeur, sur la ligne médiane
        assert!(alpha(&atlas, Tex::Streak, (0.7 * S) as usize, mid) > 200);
        assert!(alpha(&atlas, Tex::Streak, 4, mid) < 20);
        // pièce : disque percé d'un anneau
        assert_eq!(alpha(&atlas, Tex::Coin, mid, mid), 255);
        assert_eq!(alpha(&atlas, Tex::Coin, mid + (0.26 * S) as usize, mid), 0);
    }
}
