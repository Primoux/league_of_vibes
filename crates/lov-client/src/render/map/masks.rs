//! Masques du sol (r voie, g sable, b dallage, a occlusion) : portage de `paintMasks`
//! (`packages/client/src/render/map/terrain.ts`).
//!
//! Le client web les peint dans un canvas 2D (traits, disques, polygones, `filter: blur()`,
//! `globalAlpha`). Ici, un petit rastériseur logiciel fait la même chose : chaque appel de dessin est
//! rastérisé avec anticrénelage dans un calque, flouté (gaussienne d'écart type `blur` pixels, comme le
//! `blur()` CSS), puis composé en blanc sur le masque (`source-over`). Les masques sont flous : le seuil
//! 0,5 du shader tombe sur le bord réel, et le bruit qui le décale donne des contours organiques.
//!
//! Ligne 0 = bas de la carte (y = 0), comme la `DataTexture` retournée du client web.

use lov_shared::constants::MAP_SIZE;
use lov_shared::map::{
    BRUSHES, FOUNTAINS, LANE_HALF_WIDTH, LANE_PATHS, PITS, RIVER_HALF_WIDTH, WALLS,
    point_in_polygon,
};
use lov_shared::types::Vec2;

/// Résolution des masques.
pub const TEX: usize = 1024;
/// Pixels par unité de jeu.
const K: f64 = TEX as f64 / MAP_SIZE;

/// Forme d'un appel de dessin, en pixels (y vers le haut).
enum Shape<'a> {
    /// Trait à jointures et extrémités rondes (`lineJoin = lineCap = 'round'`).
    RoundStroke {
        points: &'a [Vec2],
        half: f64,
    },
    /// Segment à extrémités franches (`lineCap = 'butt'`).
    ButtSegment {
        a: Vec2,
        b: Vec2,
        half: f64,
    },
    Disc {
        center: Vec2,
        radius: f64,
    },
    Polygon(&'a [Vec2]),
}

fn px(p: Vec2) -> Vec2 {
    Vec2 {
        x: p.x * K,
        y: p.y * K,
    }
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    super::geometry::dist_to_segment(p.x, p.y, a, b)
}

/// Couverture d'un bord à distance signée `d` (positive à l'intérieur), sur un pixel.
fn edge(d: f64) -> f32 {
    (d + 0.5).clamp(0.0, 1.0) as f32
}

impl Shape<'_> {
    /// Boîte englobante en pixels (x0, y0, x1, y1).
    fn bounds(&self) -> (f64, f64, f64, f64) {
        let of = |pts: &[Vec2], pad: f64| {
            let (x0, y0, x1, y1) = pts.iter().fold(
                (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
                |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
            );
            (x0 - pad, y0 - pad, x1 + pad, y1 + pad)
        };
        match self {
            Shape::RoundStroke { points, half } => of(points, *half),
            Shape::ButtSegment { a, b, half } => of(&[*a, *b], *half),
            Shape::Disc { center, radius } => of(&[*center], *radius),
            Shape::Polygon(points) => of(points, 0.0),
        }
    }

    /// Couverture du pixel de centre `p`.
    fn coverage(&self, p: Vec2) -> f32 {
        match self {
            Shape::RoundStroke { points, half } => {
                let d = points
                    .windows(2)
                    .map(|w| seg_dist(p, w[0], w[1]))
                    .fold(f64::INFINITY, f64::min);
                edge(half - d)
            }
            Shape::ButtSegment { a, b, half } => {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let len = dx.hypot(dy);
                let along = ((p.x - a.x) * dx + (p.y - a.y) * dy) / len;
                let across = ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / len;
                edge(half - across).min(edge(along.min(len - along)))
            }
            Shape::Disc { center, radius } => edge(radius - (p.x - center.x).hypot(p.y - center.y)),
            Shape::Polygon(points) => {
                // suréchantillonnage 4 × 4
                let mut inside = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let q = Vec2 {
                            x: p.x - 0.375 + f64::from(sx) * 0.25,
                            y: p.y - 0.375 + f64::from(sy) * 0.25,
                        };
                        inside += i32::from(point_in_polygon(q, points));
                    }
                }
                inside as f32 / 16.0
            }
        }
    }
}

/// Un masque en niveaux de gris (0..1) et ses calques de travail.
struct Mask {
    value: Vec<f32>,
    layer: Vec<f32>,
    tmp: Vec<f32>,
}

impl Mask {
    fn new() -> Self {
        Self {
            value: vec![0.0; TEX * TEX],
            layer: vec![0.0; TEX * TEX],
            tmp: vec![0.0; TEX * TEX],
        }
    }

    /// Un appel de dessin en blanc : forme (en unités de jeu, déjà convertie), flou, opacité.
    fn draw(&mut self, shape: &Shape, blur: f64, alpha: f32) {
        let reach = (blur * 3.0).ceil();
        let (x0, y0, x1, y1) = shape.bounds();
        let clampi = |v: f64| v.clamp(0.0, TEX as f64) as usize;
        let (bx0, by0) = (clampi(x0 - reach - 1.0), clampi(y0 - reach - 1.0));
        let (bx1, by1) = (clampi(x1 + reach + 2.0), clampi(y1 + reach + 2.0));
        if bx0 >= bx1 || by0 >= by1 {
            return;
        }
        for y in by0..by1 {
            for x in bx0..bx1 {
                let p = Vec2 {
                    x: x as f64 + 0.5,
                    y: y as f64 + 0.5,
                };
                self.layer[y * TEX + x] = shape.coverage(p);
            }
        }
        self.blur(blur, (bx0, by0, bx1, by1));
        for y in by0..by1 {
            for x in bx0..bx1 {
                let i = y * TEX + x;
                let a = self.layer[i] * alpha;
                self.value[i] = a + self.value[i] * (1.0 - a);
                self.layer[i] = 0.0;
            }
        }
    }

    /// Flou gaussien séparable du calque, limité à la boîte (hors boîte, le calque est nul).
    fn blur(&mut self, sigma: f64, (x0, y0, x1, y1): (usize, usize, usize, usize)) {
        let r = (sigma * 3.0).ceil() as isize;
        let weights: Vec<f32> = (-r..=r)
            .map(|i| (-(i * i) as f64 / (2.0 * sigma * sigma)).exp() as f32)
            .collect();
        let total: f32 = weights.iter().sum();
        let weights: Vec<f32> = weights.iter().map(|w| w / total).collect();
        let at = |v: usize, d: isize, lo: usize, hi: usize| {
            let i = v as isize + d;
            (i >= lo as isize && i < hi as isize).then_some(i as usize)
        };
        for y in y0..y1 {
            for x in x0..x1 {
                let mut s = 0.0;
                for (k, w) in weights.iter().enumerate() {
                    if let Some(xx) = at(x, k as isize - r, x0, x1) {
                        s += self.layer[y * TEX + xx] * w;
                    }
                }
                self.tmp[y * TEX + x] = s;
            }
        }
        for y in y0..y1 {
            for x in x0..x1 {
                let mut s = 0.0;
                for (k, w) in weights.iter().enumerate() {
                    if let Some(yy) = at(y, k as isize - r, y0, y1) {
                        s += self.tmp[yy * TEX + x] * w;
                    }
                }
                self.layer[y * TEX + x] = s;
            }
        }
    }
}

fn scaled(poly: &[Vec2]) -> Vec<Vec2> {
    poly.iter().copied().map(px).collect()
}

/// Masques RGBA8 (r voie, g sable, b dallage, a occlusion), `TEX × TEX`, ligne 0 en bas.
pub fn paint_masks() -> Vec<u8> {
    let mut lane = Mask::new();
    for path in [&LANE_PATHS.top, &LANE_PATHS.mid, &LANE_PATHS.bot] {
        let points = scaled(path);
        let half = LANE_HALF_WIDTH * K;
        lane.draw(
            &Shape::RoundStroke {
                points: &points,
                half,
            },
            5.0,
            1.0,
        );
    }
    let mut sand = Mask::new();
    let river = Shape::ButtSegment {
        a: px(Vec2 {
            x: 1350.0,
            y: 10650.0,
        }),
        b: px(Vec2 {
            x: 10650.0,
            y: 1350.0,
        }),
        half: (RIVER_HALF_WIDTH + 90.0) * K,
    };
    sand.draw(&river, 7.0, 1.0);
    for pit in [PITS.dragon, PITS.leviathan] {
        let disc = Shape::Disc {
            center: px(pit.pos),
            radius: pit.radius * 0.9 * K,
        };
        sand.draw(&disc, 7.0, 1.0);
    }
    let mut paving = Mask::new();
    for f in [FOUNTAINS.blue, FOUNTAINS.red] {
        let disc = Shape::Disc {
            center: px(f),
            radius: 2330.0 * K,
        };
        paving.draw(&disc, 2.0, 1.0);
    }
    let mut occl = Mask::new();
    for w in WALLS.iter() {
        occl.draw(&Shape::Polygon(&scaled(w)), 6.0, 0.75);
    }
    for b in BRUSHES.iter() {
        occl.draw(&Shape::Polygon(&scaled(b)), 3.0, 0.4);
    }
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut data = vec![0u8; TEX * TEX * 4];
    for i in 0..TEX * TEX {
        data[i * 4] = byte(lane.value[i]);
        data[i * 4 + 1] = byte(sand.value[i]);
        data[i * 4 + 2] = byte(paving.value[i]);
        data[i * 4 + 3] = byte(occl.value[i]);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Texel du masque au point de jeu `(x, y)`.
    fn texel(data: &[u8], x: f64, y: f64) -> [u8; 4] {
        let (i, j) = ((x * K) as usize, (y * K) as usize);
        let k = (j * TEX + i) * 4;
        [data[k], data[k + 1], data[k + 2], data[k + 3]]
    }

    #[test]
    fn chaque_masque_couvre_sa_zone() {
        let data = paint_masks();
        // milieu de la voie du milieu (rivière en dessous) : voie et sable
        let mid = texel(&data, 6000.0, 6000.0);
        assert_eq!(mid[0], 255);
        assert_eq!(mid[1], 255);
        // jungle loin de tout : rien
        assert_eq!(texel(&data, 3600.0, 5200.0)[..3], [0, 0, 0]);
        // fontaine bleue : dallage, pas de sable
        let fountain = texel(&data, FOUNTAINS.blue.x, FOUNTAINS.blue.y);
        assert_eq!(fountain[2], 255);
        assert_eq!(fountain[1], 0);
        // fosse du dragon : sable
        assert_eq!(texel(&data, PITS.dragon.pos.x, PITS.dragon.pos.y)[1], 255);
        // bord de la voie du haut (axe en x ≈ 1193 à y = 6000) : mi-teinte à mi-largeur, le seuil 0,5 du
        // shader tombe sur le bord réel
        let axis = 1300.0 - 200.0 * (4000.0 / 7500.0);
        let edge = texel(&data, axis + LANE_HALF_WIDTH, 6000.0)[0];
        assert!((100..=155).contains(&edge), "bord de voie : {edge}");
    }

    #[test]
    fn occlusion_au_pied_des_murs_et_dans_les_brousses() {
        let data = paint_masks();
        let inside = |poly: &[Vec2]| {
            let c = poly.iter().fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
            let n = poly.len() as f64;
            Vec2 {
                x: c.0 / n,
                y: c.1 / n,
            }
        };
        let wall = WALLS
            .iter()
            .map(|w| inside(w))
            .find(|c| lov_shared::map::is_in_wall(*c))
            .unwrap();
        let a = texel(&data, wall.x, wall.y)[3];
        assert!((185..=192).contains(&a), "occlusion d'un mur : {a}");
    }
}
