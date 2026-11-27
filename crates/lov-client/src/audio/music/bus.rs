//! Bus de mixage de la musique (`MusicBus`) : sec + réverbération de salle générée + délai stéréo,
//! sous un gain de fondu, compressé.
//!
//! Le bus vit sur le fil audio, dans le [`Mixer`](super::super::mixer::Mixer). Le jeu le pilote par
//! [`BusCommand`] et lit son horloge par un [`MusicBusHandle`] (l'équivalent de `ctx.currentTime`) pour
//! programmer des notes à un instant absolu, sans gigue : une note arrivée en retard part tout de suite,
//! ses automatisations restant calées sur leur instant, comme en WebAudio.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::super::synth::{Biquad, FilterKind, Noise, SAMPLE_RATE};
use super::note::{Note, Score};
use super::reverb::Reverb;

const SR: f64 = SAMPLE_RATE as f64;
/// Notes simultanées au plus : la liste est préallouée, une note de plus est ignorée plutôt que de
/// réallouer sur le fil audio (un jingle de niveau 9 en compte une trentaine).
pub const MAX_NOTES: usize = 512;
/// Gain de sortie de la réverbération (`verbOut`).
const VERB_OUT: f32 = 0.55;
/// Gain de sortie du délai (`echoOut`).
const ECHO_OUT: f32 = 0.35;

/// Commande du jeu vers un bus.
#[derive(Debug)]
pub enum BusCommand {
    /// Programme une phrase (instants absolus, en temps du bus).
    Play(Score),
    /// `fade(to, seconds)` : rampe linéaire du gain global depuis sa valeur courante.
    Fade { to: f32, seconds: f64 },
    /// `master.gain.value = v` : gain global immédiat.
    SetMaster(f32),
    /// `setDelay(seconds)` : temps du délai ping-pong, rejoint avec une constante de 0,1 s.
    SetDelay(f64),
}

/// Côté jeu : identifiant du bus et lecture de son horloge.
#[derive(Debug, Clone)]
pub struct MusicBusHandle {
    id: u32,
    clock: Arc<AtomicU64>,
}

impl MusicBusHandle {
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Temps du bus (secondes), avancé à chaque bloc rendu : `ctx.currentTime`.
    pub fn current_time(&self) -> f64 {
        self.clock.load(Ordering::Relaxed) as f64 / SR
    }
}

// --------------------------------------------------------------------------------------- Ping-pong

/// Délai ping-pong : gauche → passe-bas 2,6 kHz → droite → réinjection 0,32 → gauche.
#[derive(Debug)]
struct PingPong {
    left: Box<[f32]>,
    right: Box<[f32]>,
    write: usize,
    /// Retard courant et visé, en échantillons.
    delay: f64,
    target: f64,
    tone: Biquad,
}

/// `createDelay(2)` : deux secondes de mémoire.
const DELAY_MAX: usize = 2 * SAMPLE_RATE as usize;
const FEEDBACK: f32 = 0.32;

impl PingPong {
    fn new() -> Self {
        let mut tone = Biquad::new(FilterKind::Lowpass, Biquad::DEFAULT_Q);
        tone.set_freq(2600.0);
        Self {
            left: vec![0.0; DELAY_MAX + 2].into_boxed_slice(),
            right: vec![0.0; DELAY_MAX + 2].into_boxed_slice(),
            write: 0,
            delay: 0.375 * SR,
            target: 0.375 * SR,
            tone,
        }
    }

    fn set_delay(&mut self, seconds: f64) {
        self.target = (seconds * SR).clamp(1.0, DELAY_MAX as f64);
    }

    /// Lecture interpolée linéairement à `delay` échantillons derrière l'écriture.
    fn read(buf: &[f32], write: usize, delay: f64) -> f32 {
        let len = buf.len() as f64;
        let pos = (write as f64 - delay).rem_euclid(len);
        let i = pos as usize % buf.len();
        let j = (i + 1) % buf.len();
        let frac = (pos - pos.floor()) as f32;
        buf[i] + (buf[j] - buf[i]) * frac
    }

    fn process(&mut self, x: f32) -> [f32; 2] {
        // setTargetAtTime(seconds, now, 0.1)
        const K: f64 = 1.0 / (0.1 * SR);
        self.delay += (self.target - self.delay) * (1.0 - (-K).exp());
        let l = Self::read(&self.left, self.write, self.delay);
        let r = Self::read(&self.right, self.write, self.delay);
        self.left[self.write] = x + FEEDBACK * r;
        self.right[self.write] = self.tone.process(f64::from(l)) as f32;
        self.write = (self.write + 1) % self.left.len();
        [l, r]
    }
}

// -------------------------------------------------------------------------------------- Compresseur

/// `DynamicsCompressorNode` (seuil −16 dB, genou 12 dB, rapport 3,5, attaque 10 ms, relâchement 300 ms).
///
/// Courbe statique de la spécification : identité sous le seuil, pente `1 / ratio` après le genou ; la
/// forme du genou, laissée au navigateur, est celle de Chromium (exponentielle calée sur la pente
/// `1 / ratio`). Gain de rattrapage de la spécification : `(1 / courbe(1))^0.6`. Écart : la dynamique est
/// un simple lissage attaque / relâchement du gain, sans l'anticipation de 6 ms ni le relâchement adaptatif.
#[derive(Debug)]
struct Compressor {
    k: f64,
    linear_threshold: f64,
    knee_threshold: f64,
    knee_threshold_db: f64,
    y_knee_db: f64,
    slope: f64,
    makeup: f64,
    attack: f64,
    release: f64,
    /// Réduction de gain courante (dB, ≤ 0).
    env_db: f64,
}

fn db_to_lin(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

fn lin_to_db(x: f64) -> f64 {
    20.0 * x.max(1e-12).log10()
}

impl Compressor {
    fn new(threshold: f64, knee: f64, ratio: f64, attack: f64, release: f64) -> Self {
        let mut c = Self {
            k: 5.0,
            linear_threshold: db_to_lin(threshold),
            knee_threshold: db_to_lin(threshold + knee),
            knee_threshold_db: threshold + knee,
            y_knee_db: 0.0,
            slope: 1.0 / ratio,
            makeup: 1.0,
            attack: (-1.0 / (attack * SR)).exp(),
            release: (-1.0 / (release * SR)).exp(),
            env_db: 0.0,
        };
        // k tel que la pente du genou à son extrémité vaille 1 / ratio (recherche géométrique)
        let (mut lo, mut hi) = (0.1_f64, 10_000.0_f64);
        for _ in 0..15 {
            if c.slope_at(c.knee_threshold) < c.slope {
                hi = c.k;
            } else {
                lo = c.k;
            }
            c.k = (lo * hi).sqrt();
        }
        c.y_knee_db = lin_to_db(c.knee_curve(c.knee_threshold));
        c.makeup = (1.0 / c.saturate(1.0)).powf(0.6);
        c
    }

    fn knee_curve(&self, x: f64) -> f64 {
        if x < self.linear_threshold {
            return x;
        }
        self.linear_threshold + (1.0 - (-self.k * (x - self.linear_threshold)).exp()) / self.k
    }

    fn slope_at(&self, x: f64) -> f64 {
        if x < self.linear_threshold {
            return 1.0;
        }
        let x2 = x * 1.001;
        let dy = lin_to_db(self.knee_curve(x2)) - lin_to_db(self.knee_curve(x));
        dy / (lin_to_db(x2) - lin_to_db(x))
    }

    fn saturate(&self, x: f64) -> f64 {
        if x < self.knee_threshold {
            self.knee_curve(x)
        } else {
            db_to_lin(self.y_knee_db + self.slope * (lin_to_db(x) - self.knee_threshold_db))
        }
    }

    fn process(&mut self, [l, r]: [f32; 2]) -> [f32; 2] {
        let x = f64::from(l.abs().max(r.abs()));
        let target = if x <= self.linear_threshold {
            0.0
        } else {
            lin_to_db(self.saturate(x) / x)
        };
        let k = if target < self.env_db {
            self.attack
        } else {
            self.release
        };
        self.env_db = target + (self.env_db - target) * k;
        let g = (db_to_lin(self.env_db) * self.makeup) as f32;
        [l * g, r * g]
    }
}

// ------------------------------------------------------------------------------------------- Bus

/// Gain global : `cancelScheduledValues` + `setValueAtTime(valeur courante)` + rampe linéaire.
#[derive(Debug, Clone, Copy)]
struct Fade {
    from: f32,
    to: f32,
    t0: f64,
    t1: f64,
}

impl Fade {
    fn value_at(&self, t: f64) -> f32 {
        if t >= self.t1 {
            self.to
        } else if t <= self.t0 {
            self.from
        } else {
            self.from + (self.to - self.from) * ((t - self.t0) / (self.t1 - self.t0)) as f32
        }
    }
}

/// Le bus, côté fil audio.
#[derive(Debug)]
pub struct MusicBus {
    id: u32,
    notes: Vec<Note>,
    /// Boucle de bruit de 2 s partagée par toutes les sources de bruit (`noiseBuf`).
    noise: Box<[f32]>,
    reverb: Reverb,
    echo: PingPong,
    comp: Compressor,
    master: Fade,
    /// Échantillons rendus depuis la création.
    n: u64,
    clock: Arc<AtomicU64>,
}

impl MusicBus {
    /// Construit le bus (allocations comprises : à faire hors du fil audio) et sa poignée côté jeu.
    /// Le gain global part de 0, comme le web : il faut un [`BusCommand::Fade`] ou un `SetMaster`.
    pub fn new(id: u32) -> (Self, MusicBusHandle) {
        let clock = Arc::new(AtomicU64::new(0));
        // Écart : le web tire la boucle de bruit avec `Math.random()`, ici un bruit semé.
        let mut rng = Noise::new(0xb0c1_e000 ^ id);
        let noise = (0..2 * SAMPLE_RATE).map(|_| rng.next() as f32).collect();
        let bus = Self {
            id,
            notes: Vec::with_capacity(MAX_NOTES),
            noise,
            reverb: Reverb::default(),
            echo: PingPong::new(),
            comp: Compressor::new(-16.0, 12.0, 3.5, 0.01, 0.3),
            master: Fade {
                from: 0.0,
                to: 0.0,
                t0: 0.0,
                t1: 0.0,
            },
            n: 0,
            clock: clock.clone(),
        };
        (bus, MusicBusHandle { id, clock })
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    /// Temps du bus (secondes).
    pub fn now(&self) -> f64 {
        self.n as f64 / SR
    }

    #[cfg(test)]
    pub fn active(&self) -> usize {
        self.notes.len()
    }

    pub fn command(&mut self, cmd: BusCommand) {
        let now = self.now();
        match cmd {
            BusCommand::Play(score) => {
                for note in score.notes {
                    if self.notes.len() < self.notes.capacity() {
                        self.notes.push(note);
                    }
                }
            }
            BusCommand::Fade { to, seconds } => {
                let from = self.master.value_at(now);
                self.master = Fade {
                    from,
                    to,
                    t0: now,
                    t1: now + seconds.max(0.0),
                };
            }
            BusCommand::SetMaster(v) => {
                self.master = Fade {
                    from: v,
                    to: v,
                    t0: now,
                    t1: now,
                };
            }
            BusCommand::SetDelay(s) => self.echo.set_delay(s),
        }
    }

    /// Échantillon stéréo suivant.
    pub fn next_frame(&mut self) -> [f32; 2] {
        let t = self.now();
        self.n += 1;
        let (mut dry, mut verb, mut echo) = (0.0, 0.0, 0.0);
        for note in &mut self.notes {
            let s = note.next(t, &self.noise) as f32;
            dry += s;
            verb += s * note.verb;
            echo += s * note.echo;
        }
        let [wl, wr] = self.reverb.process(verb);
        let [el, er] = self.echo.process(echo);
        let g = self.master.value_at(t);
        let l = (dry + wl * VERB_OUT + el * ECHO_OUT) * g;
        let r = (dry + wr * VERB_OUT + er * ECHO_OUT) * g;
        self.comp.process([l, r])
    }

    /// Ajoute `out.len() / 2` trames stéréo entrelacées, multipliées par `gain` (volume du canal),
    /// puis retire les notes terminées et publie l'horloge.
    pub fn render_add(&mut self, out: &mut [f32], gain: f32) {
        for frame in out.as_chunks_mut::<2>().0 {
            let [l, r] = self.next_frame();
            frame[0] += l * gain;
            frame[1] += r * gain;
        }
        let t = self.now();
        self.notes.retain(|n| !n.done(t));
        self.clock.store(self.n, Ordering::Relaxed);
    }
}
