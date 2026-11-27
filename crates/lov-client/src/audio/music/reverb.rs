//! Réverbération de salle du bus de musique : `makeImpulse` et le `ConvolverNode` du client web.
//!
//! Convoluer directement une impulsion de 3,2 s (153 600 échantillons par canal) coûterait des
//! milliards d'opérations par seconde. [`Convolver`] découpe l'impulsion en blocs de [`BLOCK`]
//! échantillons et convolue dans le domaine fréquentiel (convolution partitionnée uniforme,
//! « overlap-save ») avec une petite FFT radix-2 ([`Fft`]) : un bloc d'entrée coûte une FFT directe
//! (l'entrée est mono), puis par canal un produit-somme sur les spectres et une FFT inverse.
//!
//! Le prix est un bloc de latence ; l'impulsion commençant par 12 ms de silence (576 échantillons, plus
//! qu'un bloc), [`Reverb`] convolue avec l'impulsion privée de ses [`BLOCK`] premiers zéros : la réponse
//! totale est l'impulsion **exacte**, sans retard ajouté.
//!
//! Tout est alloué à la construction (sur le fil du jeu) ; [`Convolver::process`] n'alloue rien.

use std::f64::consts::TAU;

use super::super::synth::{Noise, SAMPLE_RATE};

/// Taille d'une partition (échantillons) : la FFT en fait le double.
pub const BLOCK: usize = 512;
const FFT_LEN: usize = 2 * BLOCK;
/// Spectre d'un signal réel : seules les fréquences 0..=N/2 sont stockées (symétrie hermitienne).
const BINS: usize = BLOCK + 1;

/// Durée et décroissance de l'impulsion (`makeImpulse(ctx, 3.2, 2.6)`).
pub const IMPULSE_SECONDS: f64 = 3.2;
pub const IMPULSE_DECAY: f64 = 2.6;
/// Pré-délai de l'impulsion : `i < sampleRate * 0.012`, soit 576 échantillons à 48 kHz.
pub const PRE_DELAY: usize = SAMPLE_RATE as usize * 12 / 1000;
// La latence d'un bloc est absorbée par le pré-délai : il doit être au moins aussi long.
const _: () = assert!(BLOCK <= PRE_DELAY);

// ------------------------------------------------------------------------------------------- FFT

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct C32 {
    re: f32,
    im: f32,
}

impl C32 {
    fn mul(self, o: C32) -> C32 {
        C32 {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }
}

/// FFT complexe radix-2 itérative, en place (Cooley-Tukey) ; tables de facteurs précalculées.
#[derive(Debug)]
struct Fft {
    rev: Box<[usize]>,
    twiddles: Box<[C32]>,
}

impl Fft {
    fn new(n: usize) -> Self {
        assert!(n.is_power_of_two() && n >= 2);
        let bits = n.trailing_zeros();
        let rev = (0..n)
            .map(|i| i.reverse_bits() >> (usize::BITS - bits))
            .collect();
        let twiddles = (0..n / 2)
            .map(|k| {
                let a = -TAU * k as f64 / n as f64;
                C32 {
                    re: a.cos() as f32,
                    im: a.sin() as f32,
                }
            })
            .collect();
        Self { rev, twiddles }
    }

    /// Transformée directe (`inverse = false`) ou inverse **non normalisée** (à diviser par N).
    fn run(&self, buf: &mut [C32], inverse: bool) {
        let n = self.rev.len();
        for i in 0..n {
            let j = self.rev[i];
            if j > i {
                buf.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let step = n / len;
            for chunk in buf.chunks_exact_mut(len) {
                let (a, b) = chunk.split_at_mut(half);
                for (k, (x, y)) in a.iter_mut().zip(b.iter_mut()).enumerate() {
                    let mut w = self.twiddles[k * step];
                    if inverse {
                        w.im = -w.im;
                    }
                    let t = y.mul(w);
                    *y = C32 {
                        re: x.re - t.re,
                        im: x.im - t.im,
                    };
                    *x = C32 {
                        re: x.re + t.re,
                        im: x.im + t.im,
                    };
                }
            }
            len *= 2;
        }
    }
}

// ------------------------------------------------------------------------------------- Convolver

/// Convolution partitionnée uniforme d'une entrée mono par une impulsion stéréo.
///
/// La sortie a [`BLOCK`] échantillons de retard : `y[n + BLOCK] = (x * h)[n]`.
#[derive(Debug)]
pub struct Convolver {
    fft: Fft,
    parts: usize,
    /// Spectres des partitions de l'impulsion, par canal : `parts × BINS`.
    h: [Box<[C32]>; 2],
    /// Spectres des derniers blocs d'entrée (ligne à retard fréquentielle, anneau de `parts` blocs).
    fdl: Box<[C32]>,
    /// Partition la plus récente dans `fdl`.
    head: usize,
    /// Fenêtre d'entrée : bloc précédent puis bloc en cours de remplissage.
    input: Box<[f32]>,
    fill: usize,
    out: [Box<[f32]>; 2],
    work: Box<[C32]>,
    acc: Box<[C32]>,
    /// Le bloc en cours a reçu un échantillon non nul.
    signal: bool,
    /// Blocs nuls consécutifs : au-delà de `parts + 1`, tout l'historique est nul, le calcul est sauté.
    silent: usize,
}

impl Convolver {
    pub fn new(ir: [&[f32]; 2]) -> Self {
        let len = ir[0].len().max(ir[1].len());
        let parts = len.div_ceil(BLOCK).max(1);
        let fft = Fft::new(FFT_LEN);
        let mut work = vec![C32::default(); FFT_LEN].into_boxed_slice();
        let h = ir.map(|ch| {
            let mut spectra = vec![C32::default(); parts * BINS].into_boxed_slice();
            for (p, dst) in spectra.as_chunks_mut::<BINS>().0.iter_mut().enumerate() {
                work.fill(C32::default());
                let from = (p * BLOCK).min(ch.len());
                let to = ((p + 1) * BLOCK).min(ch.len());
                for (w, &x) in work.iter_mut().zip(&ch[from..to]) {
                    w.re = x;
                }
                fft.run(&mut work, false);
                dst.copy_from_slice(&work[..BINS]);
            }
            spectra
        });
        Self {
            fft,
            parts,
            h,
            fdl: vec![C32::default(); parts * BINS].into_boxed_slice(),
            head: 0,
            input: vec![0.0; FFT_LEN].into_boxed_slice(),
            fill: 0,
            out: [(); 2].map(|_| vec![0.0; BLOCK].into_boxed_slice()),
            work,
            acc: vec![C32::default(); BINS].into_boxed_slice(),
            signal: false,
            silent: usize::MAX / 2,
        }
    }

    /// Un échantillon d'entrée, un échantillon stéréo de sortie (avec [`BLOCK`] de retard).
    pub fn process(&mut self, x: f32) -> [f32; 2] {
        let i = self.fill;
        self.input[BLOCK + i] = x;
        self.signal |= x != 0.0;
        let y = [self.out[0][i], self.out[1][i]];
        self.fill += 1;
        if self.fill == BLOCK {
            self.fill = 0;
            self.block();
        }
        y
    }

    fn block(&mut self) {
        self.silent = if self.signal { 0 } else { self.silent + 1 };
        self.signal = false;
        if self.silent > self.parts + 1 {
            // Les `parts + 1` derniers blocs, déjà calculés, étaient nuls : fenêtre, historique et sortie
            // le sont aussi, et le restent tant que l'entrée est nulle (bus au repos).
            return;
        }
        for (w, &x) in self.work.iter_mut().zip(self.input.iter()) {
            *w = C32 { re: x, im: 0.0 };
        }
        self.fft.run(&mut self.work, false);
        self.head = (self.head + self.parts - 1) % self.parts;
        self.fdl[self.head * BINS..][..BINS].copy_from_slice(&self.work[..BINS]);
        self.input.copy_within(BLOCK.., 0);

        for ch in 0..2 {
            self.acc.fill(C32::default());
            for p in 0..self.parts {
                // la partition `p` de l'impulsion rencontre l'entrée d'il y a `p` blocs
                let slot = (self.head + p) % self.parts;
                let x = &self.fdl[slot * BINS..][..BINS];
                let h = &self.h[ch][p * BINS..][..BINS];
                for ((a, x), h) in self.acc.iter_mut().zip(x).zip(h) {
                    a.re += x.re * h.re - x.im * h.im;
                    a.im += x.re * h.im + x.im * h.re;
                }
            }
            self.work[..BINS].copy_from_slice(&self.acc);
            for k in 1..BLOCK {
                let c = self.acc[k];
                self.work[FFT_LEN - k] = C32 {
                    re: c.re,
                    im: -c.im,
                };
            }
            self.fft.run(&mut self.work, true);
            // overlap-save : seule la seconde moitié de la convolution circulaire est valide
            let scale = 1.0 / FFT_LEN as f32;
            for (o, w) in self.out[ch].iter_mut().zip(&self.work[BLOCK..]) {
                *o = w.re * scale;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------- Impulse

/// Impulsion stéréo de `makeImpulse` : bruit à décroissance `(1 − k)^decay` après 12 ms de silence.
///
/// Écart : le web tire `Math.random()` ; ici un bruit semé par canal, la salle est identique à chaque
/// lancement (et les tests reproductibles).
pub fn make_impulse(seconds: f64, decay: f64) -> [Vec<f32>; 2] {
    let len = (f64::from(SAMPLE_RATE) * seconds).floor() as usize;
    [0x5a11_e0a1_u32, 0x5a11_e0a2].map(|seed| {
        let mut rng = Noise::new(seed);
        (0..len)
            .map(|i| {
                let k = i as f64 / len as f64;
                if i < PRE_DELAY {
                    0.0
                } else {
                    (rng.next() * (1.0 - k).powf(decay)) as f32
                }
            })
            .collect()
    })
}

/// Facteur de normalisation d'un `ConvolverNode` (`normalize = true`, valeur par défaut que le web garde) :
/// algorithme `calculateNormalizationScale` de la spécification WebAudio.
pub fn normalization_scale(ir: &[Vec<f32>]) -> f32 {
    const GAIN_CALIBRATION: f64 = 0.00125;
    const GAIN_CALIBRATION_SAMPLE_RATE: f64 = 44_100.0;
    const MIN_POWER: f64 = 0.000125;
    let len = ir.first().map_or(0, Vec::len);
    let sum: f64 = ir
        .iter()
        .flat_map(|ch| ch.iter())
        .map(|&s| f64::from(s) * f64::from(s))
        .sum();
    let mut power = (sum / (ir.len() * len).max(1) as f64).sqrt();
    if !power.is_finite() || power < MIN_POWER {
        power = MIN_POWER;
    }
    let mut scale =
        GAIN_CALIBRATION / power * GAIN_CALIBRATION_SAMPLE_RATE / f64::from(SAMPLE_RATE);
    if ir.len() == 4 {
        scale *= 0.5; // compensation « true stereo »
    }
    scale as f32
}

/// Le `ConvolverNode` du bus : impulsion générée, normalisée, rendue sans latence.
#[derive(Debug)]
pub struct Reverb {
    conv: Convolver,
}

impl Default for Reverb {
    fn default() -> Self {
        Self::from_impulse(make_impulse(IMPULSE_SECONDS, IMPULSE_DECAY))
    }
}

impl Reverb {
    /// `ir` doit commencer par au moins [`BLOCK`] zéros : ils compensent la latence du convolueur.
    pub fn from_impulse(mut ir: [Vec<f32>; 2]) -> Self {
        let scale = normalization_scale(&ir);
        for ch in &mut ir {
            debug_assert!(ch.iter().take(BLOCK).all(|&s| s == 0.0));
            for s in ch.iter_mut() {
                *s *= scale;
            }
        }
        let skip = |ch: &Vec<f32>| BLOCK.min(ch.len());
        Self {
            conv: Convolver::new([&ir[0][skip(&ir[0])..], &ir[1][skip(&ir[1])..]]),
        }
    }

    pub fn process(&mut self, x: f32) -> [f32; 2] {
        self.conv.process(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random(n: usize, seed: u32) -> Vec<f32> {
        let mut r = Noise::new(seed);
        (0..n).map(|_| r.next() as f32).collect()
    }

    fn run(conv: &mut Convolver, x: &[f32], extra: usize) -> [Vec<f32>; 2] {
        let mut out = [Vec::new(), Vec::new()];
        for i in 0..x.len() + extra {
            let [l, r] = conv.process(x.get(i).copied().unwrap_or(0.0));
            out[0].push(l);
            out[1].push(r);
        }
        out
    }

    fn assert_close(got: &[f32], want: &[f32], tol: f32, what: &str) {
        assert_eq!(got.len(), want.len(), "{what}");
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            assert!((g - w).abs() <= tol, "{what} [{i}] : {g} au lieu de {w}");
        }
    }

    #[test]
    fn la_fft_inverse_rend_le_signal() {
        let fft = Fft::new(64);
        let x: Vec<C32> = random(64, 3)
            .into_iter()
            .map(|re| C32 { re, im: 0.0 })
            .collect();
        let mut buf = x.clone();
        fft.run(&mut buf, false);
        // composante continue = somme des échantillons
        let sum: f32 = x.iter().map(|c| c.re).sum();
        assert!((buf[0].re - sum).abs() < 1e-4);
        fft.run(&mut buf, true);
        for (a, b) in buf.iter().zip(&x) {
            assert!((a.re / 64.0 - b.re).abs() < 1e-6 && (a.im / 64.0).abs() < 1e-6);
        }
    }

    #[test]
    fn la_reponse_impulsionnelle_est_l_impulsion_retardee_d_un_bloc() {
        // longueur non multiple d'un bloc, deux canaux différents, impulsion décalée hors d'une frontière
        let ir = [random(3 * BLOCK + 17, 11), random(2 * BLOCK + 300, 12)];
        let mut conv = Convolver::new([&ir[0], &ir[1]]);
        let offset = 3 * BLOCK / 2 + 7;
        let mut x = vec![0.0; offset + 1];
        x[offset] = 1.0;
        let out = run(&mut conv, &x, 5 * BLOCK);
        for (ch, h) in ir.iter().enumerate() {
            let start = offset + BLOCK;
            assert!(
                out[ch][..start].iter().all(|s| s.abs() < 1e-6),
                "silence avant"
            );
            assert_close(&out[ch][start..start + h.len()], h, 1e-5, "réponse");
            assert!(out[ch][start + h.len()..].iter().all(|s| s.abs() < 1e-5));
        }
    }

    #[test]
    fn egale_la_convolution_directe() {
        let ir = [random(3 * BLOCK, 21), random(BLOCK + 1, 22)];
        let x = random(4 * BLOCK + 123, 23);
        let mut conv = Convolver::new([&ir[0], &ir[1]]);
        let out = run(&mut conv, &x, 4 * BLOCK);
        for (ch, h) in ir.iter().enumerate() {
            let direct: Vec<f32> = (0..x.len() + h.len() - 1)
                .map(|n| {
                    let lo = n.saturating_sub(h.len() - 1);
                    (lo..=n.min(x.len() - 1))
                        .map(|i| f64::from(x[i]) * f64::from(h[n - i]))
                        .sum::<f64>() as f32
                })
                .collect();
            let got = &out[ch][BLOCK..BLOCK + direct.len()];
            assert_close(got, &direct, 2e-4, "convolution");
        }
    }

    #[test]
    fn au_repos_le_calcul_est_saute_et_reprend_exactement() {
        let ir = [random(2 * BLOCK, 31), random(2 * BLOCK, 32)];
        let mut conv = Convolver::new([&ir[0], &ir[1]]);
        let mut x = vec![0.0; 10 * BLOCK + 3];
        x[0] = 0.5;
        x[10 * BLOCK + 2] = 1.0;
        let out = run(&mut conv, &x, 4 * BLOCK);
        let start = 10 * BLOCK + 2 + BLOCK;
        assert_close(&out[0][start..start + ir[0].len()], &ir[0], 1e-5, "reprise");
    }

    #[test]
    fn impulsion_generee_comme_le_web() {
        let ir = make_impulse(IMPULSE_SECONDS, IMPULSE_DECAY);
        assert_eq!(ir[0].len(), 153_600);
        assert_eq!(PRE_DELAY, 576);
        for ch in &ir {
            assert!(ch[..PRE_DELAY].iter().all(|&s| s == 0.0), "pré-délai");
            assert!(ch[PRE_DELAY..PRE_DELAY + 100].iter().any(|&s| s != 0.0));
            assert!(ch.iter().all(|s| s.abs() <= 1.0));
            let head: f32 = ch[..48_000].iter().map(|s| s.abs()).sum();
            let tail: f32 = ch[105_600..].iter().map(|s| s.abs()).sum();
            assert!(tail < head * 0.05, "queue décroissante");
        }
        assert_ne!(ir[0], ir[1], "canaux décorrélés");
        assert_eq!(
            ir,
            make_impulse(IMPULSE_SECONDS, IMPULSE_DECAY),
            "déterministe"
        );
    }

    #[test]
    fn la_reverb_du_bus_rend_exactement_l_impulsion_de_3_2_s() {
        let ir = make_impulse(IMPULSE_SECONDS, IMPULSE_DECAY);
        let scale = normalization_scale(&ir);
        let mut verb = Reverb::default();
        for i in 0..ir[0].len() + BLOCK {
            let [l, r] = verb.process(if i == 0 { 1.0 } else { 0.0 });
            let want = |ch: usize| ir[ch].get(i).map_or(0.0, |s| s * scale);
            assert!(
                (l - want(0)).abs() < 1e-6 && (r - want(1)).abs() < 1e-6,
                "[{i}]"
            );
        }
    }

    #[test]
    fn la_reverb_rend_l_impulsion_normalisee_sans_latence() {
        // impulsion courte au même pré-délai, pour un test rapide
        let ir = make_impulse(0.1, IMPULSE_DECAY);
        let scale = normalization_scale(&ir);
        assert!(scale > 0.0 && scale < 1.0);
        let mut verb = Reverb::from_impulse(ir.clone());
        let out: Vec<[f32; 2]> = (0..ir[0].len() + BLOCK)
            .map(|i| verb.process(if i == 0 { 1.0 } else { 0.0 }))
            .collect();
        for ch in 0..2 {
            let got: Vec<f32> = out[..ir[ch].len()].iter().map(|f| f[ch]).collect();
            let want: Vec<f32> = ir[ch].iter().map(|s| s * scale).collect();
            assert_close(&got, &want, 1e-6, "réverbération");
        }
    }
}
