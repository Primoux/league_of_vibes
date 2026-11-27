//! Synthèse minimaliste façon WebAudio, sans dépendance : tout l'audio du jeu est généré, aucun
//! échantillon (voir `ASSETS.md`).
//!
//! Portage de `packages/client/src/audio/synth.ts`. Le client web programme des nœuds WebAudio
//! (oscillateur, filtre biquad, gain automatisé) ; ici chaque note est une [`Voice`] qui calcule ses
//! échantillons elle-même, avec les **mêmes courbes** : `setValueAtTime`, `linearRampToValueAtTime` et
//! `exponentialRampToValueAtTime` ([`Param`]), filtres biquad aux formules de la spécification WebAudio
//! ([`Biquad`]). Le temps d'une voix part de sa création (l'équivalent de `ctx.currentTime`).
//!
//! Tout est pur et déterministe (bruit semé) : les tests rendent dans un `Vec<f32>`, sans carte son.

use std::f64::consts::TAU;

/// Fréquence d'échantillonnage du moteur (Hz).
pub const SAMPLE_RATE: u32 = 48_000;
/// Fréquence de Nyquist : un oscillateur WebAudio ne monte jamais au-delà, il est borné en silence.
pub const NYQUIST: f64 = SAMPLE_RATE as f64 / 2.0;

/// Forme d'onde d'un oscillateur (`OscillatorType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave {
    Sine,
    Square,
    Sawtooth,
    Triangle,
}

/// Type de filtre biquad (`BiquadFilterType`, ceux dont le jeu se sert).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Lowpass,
    Highpass,
    Bandpass,
}

// ------------------------------------------------------------------------------------------ Param

#[derive(Debug, Clone, Copy)]
enum Ramp {
    Set,
    Linear,
    Exponential,
}

#[derive(Debug, Clone, Copy)]
struct Point {
    time: f64,
    value: f64,
    ramp: Ramp,
}

/// Nombre maximal d'événements d'automatisation par paramètre (une enveloppe en utilise quatre au plus).
const MAX_POINTS: usize = 6;

/// Paramètre automatisé (`AudioParam`) : une suite d'événements datés, évaluée à la demande.
///
/// Sémantique WebAudio : avant le premier événement, la valeur par défaut ; une rampe va de l'événement
/// précédent (instant et valeur) à la sienne ; après le dernier, sa valeur est tenue.
#[derive(Debug, Clone)]
pub struct Param {
    default: f64,
    points: [Point; MAX_POINTS],
    len: usize,
}

impl Param {
    pub fn new(default: f64) -> Self {
        let p = Point {
            time: 0.0,
            value: default,
            ramp: Ramp::Set,
        };
        Self {
            default,
            points: [p; MAX_POINTS],
            len: 0,
        }
    }

    fn push(&mut self, time: f64, value: f64, ramp: Ramp) -> &mut Self {
        // Au-delà de la capacité, l'événement est ignoré : aucune recette n'en programme autant.
        if self.len < MAX_POINTS {
            self.points[self.len] = Point { time, value, ramp };
            self.len += 1;
        }
        self
    }

    pub fn set_value_at_time(&mut self, value: f64, time: f64) -> &mut Self {
        self.push(time, value, Ramp::Set)
    }

    pub fn linear_ramp_to_value_at_time(&mut self, value: f64, time: f64) -> &mut Self {
        self.push(time, value, Ramp::Linear)
    }

    pub fn exponential_ramp_to_value_at_time(&mut self, value: f64, time: f64) -> &mut Self {
        self.push(time, value, Ramp::Exponential)
    }

    /// `true` si la valeur ne change jamais (aucune rampe) : un filtre n'a alors pas à se recalculer.
    pub fn is_constant(&self) -> bool {
        self.len <= 1
    }

    /// Valeur à l'instant `t` (secondes).
    pub fn value_at(&self, t: f64) -> f64 {
        let pts = &self.points[..self.len];
        let Some(first) = pts.first() else {
            return self.default;
        };
        // Avant le premier événement : une rampe part de la valeur par défaut à l'instant 0.
        let mut prev = Point {
            time: 0.0,
            value: self.default,
            ramp: Ramp::Set,
        };
        if t < first.time && matches!(first.ramp, Ramp::Set) {
            return self.default;
        }
        for p in pts {
            if t < p.time {
                return interpolate(prev, *p, t);
            }
            prev = *p;
        }
        prev.value
    }
}

fn interpolate(a: Point, b: Point, t: f64) -> f64 {
    let span = b.time - a.time;
    match b.ramp {
        Ramp::Set => a.value,
        _ if span <= 0.0 => b.value,
        Ramp::Linear => a.value + (b.value - a.value) * (t - a.time) / span,
        // WebAudio : rampe exponentielle impossible si les deux valeurs ne sont pas de même signe, non nulles.
        Ramp::Exponential if a.value * b.value <= 0.0 => a.value,
        Ramp::Exponential => a.value * (b.value / a.value).powf((t - a.time) / span),
    }
}

// ------------------------------------------------------------------------------------- Oscillator

/// Oscillateur à accumulateur de phase. Carré et dent de scie sont adoucis (polyBLEP) pour limiter le
/// repliement, comme les tables à bande limitée de WebAudio.
#[derive(Debug, Clone)]
pub struct Osc {
    wave: Wave,
    phase: f64,
}

impl Osc {
    pub fn new(wave: Wave) -> Self {
        Self { wave, phase: 0.0 }
    }

    /// Échantillon suivant à la fréquence `freq` (Hz), bornée à Nyquist.
    pub fn next(&mut self, freq: f64) -> f64 {
        let dt = (freq.clamp(0.0, NYQUIST) / SAMPLE_RATE as f64).min(0.5);
        let p = self.phase;
        let out = match self.wave {
            Wave::Sine => (TAU * p).sin(),
            Wave::Square => {
                let naive = if p < 0.5 { 1.0 } else { -1.0 };
                naive + poly_blep(p, dt) - poly_blep((p + 0.5) % 1.0, dt)
            }
            // WebAudio : la dent de scie part de 0 en montant (forme `2·frac(p + ½) − 1`).
            Wave::Sawtooth => {
                let q = (p + 0.5) % 1.0;
                2.0 * q - 1.0 - poly_blep(q, dt)
            }
            // Triangle : part de 0 en montant, crête à ¼ de période.
            Wave::Triangle => {
                if p < 0.25 {
                    4.0 * p
                } else if p < 0.75 {
                    2.0 - 4.0 * p
                } else {
                    4.0 * p - 4.0
                }
            }
        };
        self.phase = (p + dt) % 1.0;
        out
    }
}

fn poly_blep(t: f64, dt: f64) -> f64 {
    if dt <= 0.0 {
        0.0
    } else if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

// ----------------------------------------------------------------------------------------- Noise

/// Bruit blanc semé (xorshift32), déterministe pour une même graine.
#[derive(Debug, Clone)]
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    /// Valeur uniforme dans `[-1, 1)`.
    pub fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        f64::from(x) / f64::from(u32::MAX) * 2.0 - 1.0
    }

    /// Valeur uniforme dans `[0, 1)`, pour les variations aléatoires des recettes.
    pub fn unit(&mut self) -> f64 {
        (self.next() + 1.0) * 0.5
    }
}

// ---------------------------------------------------------------------------------------- Biquad

/// Filtre biquad (forme directe I) aux formules de la spécification WebAudio (Audio EQ Cookbook) :
/// pour passe-bas et passe-haut, `Q` est une résonance **en dB** ; pour passe-bande, un facteur linéaire.
#[derive(Debug, Clone)]
pub struct Biquad {
    kind: FilterKind,
    q: f64,
    freq: f64,
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    /// `Q` par défaut d'un `BiquadFilterNode`.
    pub const DEFAULT_Q: f64 = 1.0;

    pub fn new(kind: FilterKind, q: f64) -> Self {
        let mut f = Self {
            kind,
            q,
            freq: f64::NAN,
            b: [1.0, 0.0, 0.0],
            a: [0.0, 0.0],
            x: [0.0; 2],
            y: [0.0; 2],
        };
        f.set_freq(350.0);
        f
    }

    /// Change la fréquence de coupure ; ne recalcule les coefficients que si elle a changé.
    pub fn set_freq(&mut self, freq: f64) {
        let freq = freq.clamp(10.0, NYQUIST * 0.999);
        if freq == self.freq {
            return;
        }
        self.freq = freq;
        let w0 = TAU * freq / SAMPLE_RATE as f64;
        let (sin, cos) = w0.sin_cos();
        let alpha = match self.kind {
            FilterKind::Lowpass | FilterKind::Highpass => sin / (2.0 * 10f64.powf(self.q / 20.0)),
            FilterKind::Bandpass => sin / (2.0 * self.q.max(1e-4)),
        };
        let (b0, b1, b2) = match self.kind {
            FilterKind::Lowpass => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0),
            FilterKind::Highpass => ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0),
            FilterKind::Bandpass => (alpha, 0.0, -alpha),
        };
        let a0 = 1.0 + alpha;
        self.b = [b0 / a0, b1 / a0, b2 / a0];
        self.a = [-2.0 * cos / a0, (1.0 - alpha) / a0];
    }

    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

// ----------------------------------------------------------------------------------------- Voice

/// Options de [`tone`] (`ToneOpts`) : valeurs par défaut identiques au client web.
#[derive(Debug, Clone, Copy)]
pub struct ToneOpts {
    pub wave: Wave,
    /// Fréquence de départ (Hz).
    pub freq: f64,
    /// Glissement vers `freq * slide` (1 = aucun).
    pub slide: f64,
    /// Durée (s).
    pub dur: f64,
    /// Volume 0..1.
    pub vol: f64,
    /// Secondes avant le début.
    pub delay: f64,
    pub attack: f64,
    /// Coupure d'un passe-bas appliqué, s'il y en a un.
    pub filter: Option<f64>,
}

impl Default for ToneOpts {
    fn default() -> Self {
        Self {
            wave: Wave::Sine,
            freq: 440.0,
            slide: 1.0,
            dur: 0.15,
            vol: 0.2,
            delay: 0.0,
            attack: 0.005,
            filter: None,
        }
    }
}

/// Options de [`noise`] : valeurs par défaut identiques au client web.
#[derive(Debug, Clone, Copy)]
pub struct NoiseOpts {
    pub dur: f64,
    pub vol: f64,
    pub delay: f64,
    /// Fréquence du filtre, s'il y en a un.
    pub filter: Option<f64>,
    /// Glissement du filtre jusqu'à cette fréquence, en fin de son.
    pub slide_filter: Option<f64>,
    pub kind: FilterKind,
}

impl Default for NoiseOpts {
    fn default() -> Self {
        Self {
            dur: 0.2,
            vol: 0.2,
            delay: 0.0,
            filter: None,
            slide_filter: None,
            kind: FilterKind::Lowpass,
        }
    }
}

#[derive(Debug, Clone)]
enum Source {
    Osc { osc: Osc, freq: Param },
    Noise(Noise),
}

/// Une note programmée : source (oscillateur ou bruit), filtre optionnel, gain automatisé.
#[derive(Debug, Clone)]
pub struct Voice {
    source: Source,
    filter: Option<(Biquad, Param)>,
    gain: Param,
    start: f64,
    stop: f64,
    /// Échantillons écoulés depuis la création.
    n: u64,
}

impl Voice {
    /// `true` une fois l'instant d'arrêt passé : la voix peut être retirée.
    pub fn done(&self) -> bool {
        self.time() >= self.stop
    }

    fn time(&self) -> f64 {
        self.n as f64 / SAMPLE_RATE as f64
    }

    /// Échantillon suivant (mono). Silence avant le départ et après l'arrêt.
    pub fn next(&mut self) -> f64 {
        let t = self.time();
        self.n += 1;
        if t < self.start || t >= self.stop {
            return 0.0;
        }
        let mut s = match &mut self.source {
            Source::Osc { osc, freq } => osc.next(freq.value_at(t)),
            Source::Noise(n) => n.next(),
        };
        if let Some((f, freq)) = &mut self.filter {
            f.set_freq(freq.value_at(t));
            s = f.process(s);
        }
        s * self.gain.value_at(t)
    }
}

/// Enveloppe des deux recettes : de presque rien à `vol` en `attack`, puis retour à presque rien à `end`.
fn envelope(t0: f64, attack: f64, vol: f64, end: f64) -> Param {
    let mut g = Param::new(1.0);
    g.set_value_at_time(0.0001, t0)
        .exponential_ramp_to_value_at_time(vol.max(0.0001), t0 + attack)
        .exponential_ramp_to_value_at_time(0.0001, end);
    g
}

/// Onde (oscillateur) avec enveloppe, comme `tone()` du client web.
///
/// Le glissement est un **multiplicateur** de `freq`. Les recettes qui passent une fréquence en Hz
/// (`slide: 220` sur 110 Hz) visent donc des dizaines de kHz : WebAudio borne en silence à Nyquist, l'
/// oscillateur aussi ([`Osc::next`]). Les constantes sont portées telles quelles.
pub fn tone(o: ToneOpts) -> Voice {
    let t0 = o.delay;
    let mut freq = Param::new(o.freq);
    freq.set_value_at_time(o.freq, t0);
    if o.slide != 1.0 && o.slide != 0.0 {
        freq.exponential_ramp_to_value_at_time((o.freq * o.slide).max(20.0), t0 + o.dur);
    }
    let filter = o.filter.map(|f| {
        let mut p = Param::new(f);
        p.set_value_at_time(f, 0.0);
        (Biquad::new(FilterKind::Lowpass, Biquad::DEFAULT_Q), p)
    });
    Voice {
        source: Source::Osc {
            osc: Osc::new(o.wave),
            freq,
        },
        filter,
        gain: envelope(t0, o.attack, o.vol, t0 + o.dur),
        start: t0,
        stop: t0 + o.dur + 0.05,
        n: 0,
    }
}

/// Bruit blanc filtré (percussions, explosions, souffles), comme `noise()` du client web.
pub fn noise(o: NoiseOpts, seed: u32) -> Voice {
    let t0 = o.delay;
    let filter = o.filter.map(|f| {
        let mut p = Param::new(f);
        p.set_value_at_time(f, t0);
        if let Some(to) = o.slide_filter.filter(|&s| s != 0.0) {
            p.exponential_ramp_to_value_at_time(to.max(40.0), t0 + o.dur);
        }
        (Biquad::new(o.kind, Biquad::DEFAULT_Q), p)
    });
    Voice {
        source: Source::Noise(Noise::new(seed)),
        filter,
        gain: envelope(t0, 0.01, o.vol, t0 + o.dur),
        start: t0,
        stop: t0 + o.dur + 0.05,
        n: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(v: &mut Voice, seconds: f64) -> Vec<f64> {
        (0..(seconds * SAMPLE_RATE as f64) as usize)
            .map(|_| v.next())
            .collect()
    }

    fn rms(s: &[f64]) -> f64 {
        (s.iter().map(|x| x * x).sum::<f64>() / s.len().max(1) as f64).sqrt()
    }

    fn peak(s: &[f64]) -> f64 {
        s.iter().fold(0.0, |m, x| m.max(x.abs()))
    }

    const SR: f64 = SAMPLE_RATE as f64;

    #[test]
    fn rampes_aux_courbes_webaudio() {
        let mut p = Param::new(5.0);
        p.set_value_at_time(1.0, 1.0)
            .linear_ramp_to_value_at_time(3.0, 2.0)
            .exponential_ramp_to_value_at_time(12.0, 4.0);
        assert_eq!(p.value_at(0.5), 5.0, "avant le premier événement : défaut");
        assert_eq!(p.value_at(1.0), 1.0);
        assert!((p.value_at(1.5) - 2.0).abs() < 1e-9, "linéaire");
        assert!((p.value_at(3.0) - 6.0).abs() < 1e-9, "exponentielle : 3·√4");
        assert_eq!(p.value_at(10.0), 12.0, "dernière valeur tenue");
    }

    #[test]
    fn la_note_respecte_delai_volume_et_arret() {
        let mut v = tone(ToneOpts {
            freq: 440.0,
            dur: 0.2,
            vol: 0.5,
            delay: 0.1,
            ..ToneOpts::default()
        });
        let s = render(&mut v, 0.4);
        assert_eq!(
            peak(&s[..(0.1 * SR) as usize]),
            0.0,
            "silence avant le délai"
        );
        let body = peak(&s[(0.1 * SR) as usize..(0.2 * SR) as usize]);
        assert!(
            (0.3..=0.5).contains(&body),
            "crête proche du volume : {body}"
        );
        assert!(
            peak(&s[(0.3 * SR) as usize..]) < 1e-3,
            "éteinte après la durée"
        );
        assert!(v.done());
    }

    #[test]
    fn un_glissement_demesure_reste_borne_a_nyquist() {
        // Ult de Mélenchon : slide 220 sur 110 Hz, soit une cible de 24 kHz, au-delà de Nyquist.
        let mut v = tone(ToneOpts {
            wave: Wave::Sawtooth,
            freq: 110.0,
            slide: 220.0,
            dur: 0.6,
            vol: 0.22,
            filter: Some(900.0),
            ..ToneOpts::default()
        });
        let s = render(&mut v, 0.7);
        assert!(s.iter().all(|x| x.is_finite()));
        assert!(peak(&s) <= 0.5);
    }

    #[test]
    fn le_passe_bas_coupe_les_aigus_et_garde_les_graves() {
        let through = |hz: f64| {
            let mut osc = Osc::new(Wave::Sine);
            let mut f = Biquad::new(FilterKind::Lowpass, Biquad::DEFAULT_Q);
            f.set_freq(500.0);
            let s: Vec<f64> = (0..SAMPLE_RATE).map(|_| f.process(osc.next(hz))).collect();
            rms(&s[SAMPLE_RATE as usize / 2..]) / std::f64::consts::FRAC_1_SQRT_2
        };
        assert!((0.9..1.2).contains(&through(100.0)), "100 Hz passe");
        assert!(through(5000.0) < 0.02, "5 kHz atténué de plus de 34 dB");
    }

    #[test]
    fn passe_haut_et_passe_bande_filtrent_le_bruit() {
        let energy = |kind: FilterKind, f: f64| {
            let mut n = Noise::new(7);
            let mut b = Biquad::new(kind, Biquad::DEFAULT_Q);
            b.set_freq(f);
            let s: Vec<f64> = (0..SAMPLE_RATE).map(|_| b.process(n.next())).collect();
            rms(&s)
        };
        let raw = 1.0 / 3f64.sqrt();
        assert!(energy(FilterKind::Highpass, 3500.0) < raw);
        assert!(energy(FilterKind::Bandpass, 1800.0) < raw * 0.5);
    }

    #[test]
    fn le_bruit_est_deterministe_et_le_filtre_glisse() {
        let opts = NoiseOpts {
            dur: 0.3,
            vol: 0.3,
            filter: Some(3000.0),
            slide_filter: Some(600.0),
            ..NoiseOpts::default()
        };
        let a = render(&mut noise(opts, 42), 0.4);
        let b = render(&mut noise(opts, 42), 0.4);
        assert_eq!(a, b);
        assert!(peak(&a) > 0.05);
        assert!(
            peak(&a[(0.36 * SR) as usize..]) == 0.0,
            "coupé après dur + 50 ms"
        );
    }

    #[test]
    fn formes_d_onde_bornees_et_de_bon_signe() {
        for w in [Wave::Sine, Wave::Square, Wave::Sawtooth, Wave::Triangle] {
            let mut o = Osc::new(w);
            let s: Vec<f64> = (0..4800).map(|_| o.next(220.0)).collect();
            assert!(peak(&s) <= 1.2, "{w:?}");
            assert!(rms(&s) > 0.4, "{w:?}");
        }
    }
}
