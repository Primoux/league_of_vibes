//! Courbes et dégradés pour les particules (logique pure, testée).
//! Les styles de particules sont « cuits » dans une texture (`CURVE_RES` échantillons par ligne) lue par le GPU.
//!
//! Portage de `packages/client/src/fx/core/curves.ts`.

/// Clé de courbe : `[t ∈ 0..1, valeur]`. Les clés doivent être triées par `t` croissant.
pub type CurveKey = [f32; 2];
pub type Curve = [CurveKey];
/// Arrêt de dégradé : `[t, r, g, b]` en couleur linéaire (peut dépasser 1 pour le bloom HDR).
pub type ColorStop = [f32; 4];

pub const CURVE_RES: usize = 64;

/// Échantillonne une courbe par interpolation linéaire (constante hors des bornes).
pub fn sample_curve(c: &Curve, t: f32) -> f32 {
    let Some(first) = c.first() else {
        return 1.0;
    };
    if t <= first[0] {
        return first[1];
    }
    for w in c.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t <= b[0] {
            let span = b[0] - a[0];
            let k = if span > 0.0 { (t - a[0]) / span } else { 1.0 };
            return a[1] + (b[1] - a[1]) * k;
        }
    }
    c[c.len() - 1][1]
}

/// Échantillonne un dégradé : `[r, g, b]`.
pub fn sample_gradient(stops: &[ColorStop], t: f32) -> [f32; 3] {
    let Some(&first) = stops.first() else {
        return [1.0; 3];
    };
    let mut a = first;
    let mut b = a;
    if t > a[0] {
        b = stops[stops.len() - 1];
        for i in 1..stops.len() {
            if t <= stops[i][0] {
                a = stops[i - 1];
                b = stops[i];
                break;
            }
            a = stops[i];
        }
    }
    let span = b[0] - a[0];
    let k = if span > 0.0 {
        ((t - a[0]) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    [
        a[1] + (b[1] - a[1]) * k,
        a[2] + (b[2] - a[2]) * k,
        a[3] + (b[3] - a[3]) * k,
    ]
}

/// Écrit 2 lignes RGBA de `CURVE_RES` texels dans `out` à partir de `row` :
/// ligne `row` = couleur (rgb) + alpha, ligne `row + 1` = taille dans le canal r.
pub fn bake_style(color: &[ColorStop], alpha: &Curve, size: &Curve, out: &mut [f32], row: usize) {
    let stride = CURVE_RES * 4;
    let c0 = row * stride;
    let s0 = (row + 1) * stride;
    for i in 0..CURVE_RES {
        let t = i as f32 / (CURVE_RES - 1) as f32;
        let [r, g, b] = sample_gradient(color, t);
        out[c0 + i * 4..c0 + i * 4 + 4].copy_from_slice(&[r, g, b, sample_curve(alpha, t)]);
        out[s0 + i * 4..s0 + i * 4 + 4].copy_from_slice(&[sample_curve(size, t), 0.0, 0.0, 1.0]);
    }
}

/// Courbes usuelles (`CURVES` du client web).
pub mod presets {
    use super::CurveKey;

    pub const ONE: &[CurveKey] = &[[0.0, 1.0]];
    pub const FADE_OUT: &[CurveKey] = &[[0.0, 1.0], [1.0, 0.0]];
    pub const FADE_IN_OUT: &[CurveKey] = &[[0.0, 0.0], [0.15, 1.0], [0.7, 0.8], [1.0, 0.0]];
    pub const QUICK_IN_SLOW_OUT: &[CurveKey] = &[[0.0, 0.0], [0.05, 1.0], [1.0, 0.0]];
    pub const GROW: &[CurveKey] = &[[0.0, 0.3], [1.0, 1.0]];
    pub const SHRINK: &[CurveKey] = &[[0.0, 1.0], [1.0, 0.0]];
    pub const POP_SHRINK: &[CurveKey] = &[[0.0, 0.4], [0.1, 1.0], [1.0, 0.1]];
    pub const SMOKE_GROW: &[CurveKey] = &[[0.0, 0.4], [1.0, 1.6]];
}

/// Easings (`t ∈ 0..1`).
pub mod ease {
    use std::f32::consts::PI;

    pub fn out_cubic(t: f32) -> f32 {
        1.0 - (1.0 - t).powi(3)
    }

    pub fn in_cubic(t: f32) -> f32 {
        t * t * t
    }

    pub fn in_out_sine(t: f32) -> f32 {
        -((PI * t).cos() - 1.0) / 2.0
    }

    pub fn out_back(t: f32) -> f32 {
        let c1 = 1.70158;
        let c3 = c1 + 1.0;
        1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn interpole_lineairement_et_borne_aux_extremites() {
        let c: &Curve = &[[0.0, 0.0], [0.5, 1.0], [1.0, 0.0]];
        assert_eq!(sample_curve(c, -1.0), 0.0);
        assert!(close(sample_curve(c, 0.25), 0.5));
        assert_eq!(sample_curve(c, 0.5), 1.0);
        assert!(close(sample_curve(c, 0.75), 0.5));
        assert_eq!(sample_curve(c, 2.0), 0.0);
    }

    #[test]
    fn courbe_vide_vaut_1_courbe_a_une_cle_constante() {
        assert_eq!(sample_curve(&[], 0.3), 1.0);
        assert_eq!(sample_curve(&[[0.0, 0.4]], 0.9), 0.4);
    }

    #[test]
    fn degrade_interpolation_par_canal() {
        assert_eq!(
            sample_gradient(&[[0.0, 1.0, 0.0, 0.0], [1.0, 0.0, 0.0, 2.0]], 0.5),
            [0.5, 0.0, 1.0]
        );
        let stops = [[0.2, 1.0, 1.0, 1.0], [0.8, 0.0, 0.0, 0.0]];
        assert_eq!(sample_gradient(&stops, 0.1)[0], 1.0);
        assert_eq!(sample_gradient(&stops, 0.9)[0], 0.0);
    }

    #[test]
    fn bake_style_ecrit_couleur_alpha_puis_taille_sur_2_lignes() {
        let mut data = vec![0.0; CURVE_RES * 4 * 4];
        bake_style(
            &[[0.0, 1.0, 2.0, 3.0]],
            &[[0.0, 1.0], [1.0, 0.0]],
            &[[0.0, 0.5], [1.0, 1.0]],
            &mut data,
            2,
        );
        let row2 = 2 * CURVE_RES * 4;
        let row3 = 3 * CURVE_RES * 4;
        assert_eq!(data[row2..row2 + 4], [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(data[row2 + (CURVE_RES - 1) * 4 + 3], 0.0); // alpha final
        assert_eq!(data[row3], 0.5);
        assert_eq!(data[row3 + (CURVE_RES - 1) * 4], 1.0);
        assert_eq!(data[0], 0.0); // lignes 0-1 intactes
    }
}
