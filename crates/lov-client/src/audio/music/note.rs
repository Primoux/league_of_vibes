//! Notes des instruments : le petit graphe WebAudio qu'un instrument du client web construit à chaque
//! appel (oscillateurs désaccordés, vibrato, filtres automatisés, gains), ramené à une structure fixe.
//!
//! Une [`Note`] est la sortie d'une voix (`bus.voice(verb, echo)`) : un gain automatisé et des envois
//! vers la réverbération et le délai. Elle somme des [`Layer`]s — un filtre optionnel suivi d'un gain —
//! qui somment chacune des sources (oscillateur ou bruit), avec leurs propres instants de départ et
//! d'arrêt (`start(t)` / `stop(end)`). Tous les instants sont en **temps du bus** (secondes).

use super::super::synth::{Biquad, FilterKind, Osc, Param, Wave};

/// Une phrase : les notes à programmer d'un coup sur le bus (un seul message vers le fil audio).
#[derive(Debug, Default)]
pub struct Score {
    pub(super) notes: Vec<Note>,
}

impl Score {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    /// Instant où la dernière source de la phrase s'arrête (temps du bus), `-∞` si elle est vide.
    pub fn end(&self) -> f64 {
        self.notes
            .iter()
            .map(|n| n.stop)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    pub(super) fn push(&mut self, mut note: Note) {
        let srcs = || note.layers.iter().flat_map(|l| l.sources.iter());
        note.start = srcs().map(|s| s.start).fold(f64::INFINITY, f64::min);
        note.stop = srcs().map(|s| s.stop).fold(f64::NEG_INFINITY, f64::max);
        self.notes.push(note);
    }
}

/// Vibrato : un oscillateur sinus basse fréquence ajouté au désaccord (en cents) de l'oscillateur.
#[derive(Debug, Clone)]
struct Vibrato {
    lfo: Osc,
    rate: f64,
    depth: Param,
}

#[derive(Debug, Clone)]
enum Kind {
    Osc {
        osc: Osc,
        freq: Param,
        /// `2^(detune / 1200)` : désaccord fixe en cents.
        ratio: f64,
        vibrato: Option<Box<Vibrato>>,
    },
    /// Lecture de la boucle de bruit du bus, depuis son début (`bus.noise()`).
    Noise { pos: usize },
}

/// Une source (`OscillatorNode` ou `AudioBufferSourceNode`), avec un gain fixe.
#[derive(Debug, Clone)]
pub struct Src {
    kind: Kind,
    level: f64,
    start: f64,
    stop: f64,
}

impl Src {
    /// Gain fixe placé après la source (`GainNode` à valeur constante).
    pub fn level(&mut self, level: f64) -> &mut Self {
        self.level = level;
        self
    }

    /// Vibrato de `rate` Hz, profondeur en cents donnée par `depth` (automatisable).
    pub fn vibrato(&mut self, rate: f64, depth: Param) -> &mut Self {
        if let Kind::Osc { vibrato, .. } = &mut self.kind {
            *vibrato = Some(Box::new(Vibrato {
                lfo: Osc::new(Wave::Sine),
                rate,
                depth,
            }));
        }
        self
    }

    /// Fréquence (automatisable) d'un oscillateur.
    pub fn freq(&mut self) -> Option<&mut Param> {
        match &mut self.kind {
            Kind::Osc { freq, .. } => Some(freq),
            Kind::Noise { .. } => None,
        }
    }

    fn next(&mut self, t: f64, noise: &[f32]) -> f64 {
        if t < self.start || t >= self.stop {
            return 0.0;
        }
        let s = match &mut self.kind {
            Kind::Osc {
                osc,
                freq,
                ratio,
                vibrato,
            } => {
                let mut f = freq.value_at(t) * *ratio;
                if let Some(v) = vibrato {
                    let cents = v.lfo.next(v.rate) * v.depth.value_at(t);
                    f *= (cents / 1200.0).exp2();
                }
                osc.next(f)
            }
            Kind::Noise { pos } => {
                let s = noise.get(*pos).copied().unwrap_or(0.0);
                *pos = (*pos + 1) % noise.len().max(1);
                f64::from(s)
            }
        };
        s * self.level
    }
}

/// Sources sommées, puis un filtre optionnel (fréquence automatisable), puis un gain automatisable.
#[derive(Debug, Clone)]
pub struct Layer {
    filter: Option<(Biquad, Param)>,
    pub gain: Param,
    sources: Vec<Src>,
}

impl Layer {
    /// Sans filtre.
    pub fn plain() -> Self {
        Self {
            filter: None,
            gain: Param::new(1.0),
            sources: Vec::new(),
        }
    }

    /// Avec un filtre biquad de facteur `q` (en dB pour passe-bas et passe-haut, comme WebAudio) et de
    /// fréquence `freq` : `Param::new(f)` pour une fréquence fixe (`frequency.value = f`).
    pub fn filtered(kind: FilterKind, q: f64, freq: Param) -> Self {
        Self {
            filter: Some((Biquad::new(kind, q), freq)),
            ..Self::plain()
        }
    }

    /// Oscillateur de `start` à `stop`, désaccordé de `detune` cents (`osc()` du client web).
    pub fn osc(&mut self, wave: Wave, freq: f64, start: f64, stop: f64, detune: f64) -> &mut Src {
        let mut p = Param::new(freq);
        p.set_value_at_time(freq, start);
        self.push(Src {
            kind: Kind::Osc {
                osc: Osc::new(wave),
                freq: p,
                ratio: (detune / 1200.0).exp2(),
                vibrato: None,
            },
            level: 1.0,
            start,
            stop,
        })
    }

    /// Boucle de bruit du bus, de `start` à `stop`.
    pub fn noise(&mut self, start: f64, stop: f64) -> &mut Src {
        self.push(Src {
            kind: Kind::Noise { pos: 0 },
            level: 1.0,
            start,
            stop,
        })
    }

    fn push(&mut self, s: Src) -> &mut Src {
        self.sources.push(s);
        let last = self.sources.len() - 1;
        &mut self.sources[last]
    }

    fn next(&mut self, t: f64, noise: &[f32]) -> f64 {
        let mut s = 0.0;
        for src in &mut self.sources {
            s += src.next(t, noise);
        }
        if let Some((f, freq)) = &mut self.filter {
            f.set_freq(freq.value_at(t));
            s = f.process(s);
        }
        s * self.gain.value_at(t)
    }
}

/// Sortie d'une voix : gain automatisé et envois vers la réverbération et le délai.
#[derive(Debug, Clone)]
pub struct Note {
    pub gain: Param,
    pub(super) verb: f32,
    pub(super) echo: f32,
    layers: Vec<Layer>,
    start: f64,
    stop: f64,
}

impl Note {
    /// `bus.voice(verb, echo)` : niveaux d'envoi vers la réverbération et vers le délai.
    pub fn voice(verb: f64, echo: f64) -> Self {
        Self {
            gain: Param::new(1.0),
            verb: verb as f32,
            echo: echo as f32,
            layers: Vec::new(),
            start: 0.0,
            stop: 0.0,
        }
    }

    pub fn add(&mut self, layer: Layer) {
        self.layers.push(layer);
    }

    /// Plus aucune source ne joue à l'instant `t` : la note peut être retirée.
    pub fn done(&self, t: f64) -> bool {
        t >= self.stop
    }

    /// Échantillon (mono) à l'instant `t` du bus. Avant la première source, tout est nul.
    pub fn next(&mut self, t: f64, noise: &[f32]) -> f64 {
        if t < self.start {
            return 0.0;
        }
        let mut s = 0.0;
        for l in &mut self.layers {
            s += l.next(t, noise);
        }
        s * self.gain.value_at(t)
    }
}
