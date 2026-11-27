//! Canaux, volumes, spatialisation et limite de polyphonie.
//!
//! Portage de la partie mixage de `packages/client/src/audio/AudioManager.ts` : cinq canaux (master,
//! effets, voix et annonces, musique, interface) aux volumes réglables, atténuation selon la distance à
//! la caméra, et au plus 24 effets simultanés sur le canal des effets.
//!
//! [`Mixer`] vit sur le fil audio et rend les échantillons ; [`SfxGate`] et [`spatial_gain`] sont évalués
//! côté jeu, au moment où l'effet est demandé, comme dans le client web.

use std::collections::VecDeque;

use lov_shared::types::Vec2;

use super::music::{BusCommand, MusicBus};
use super::synth::Voice;

/// Canal de mixage (`AudioChannel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Master,
    Sfx,
    Voice,
    Music,
    Ui,
}

/// Les canaux dans l'ordre du client web (`AUDIO_CHANNELS`).
pub const AUDIO_CHANNELS: [Channel; 5] = [
    Channel::Master,
    Channel::Sfx,
    Channel::Voice,
    Channel::Music,
    Channel::Ui,
];

/// Polyphonie maximale sur le canal des effets.
pub const MAX_SFX: usize = 24;
/// Durée pendant laquelle un effet occupe une place de polyphonie (`setTimeout(cleanup, 2500)`).
pub const SFX_SLOT_SECONDS: f64 = 2.5;

impl Channel {
    fn index(self) -> usize {
        self as usize
    }
}

/// Volumes par canal (0..1) et coupure générale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volumes {
    levels: [f32; 5],
    pub muted: bool,
}

impl Default for Volumes {
    fn default() -> Self {
        let mut v = Self {
            levels: [0.0; 5],
            muted: false,
        };
        for (ch, level) in [
            (Channel::Master, 0.8),
            (Channel::Sfx, 0.9),
            (Channel::Voice, 1.0),
            (Channel::Music, 0.5),
            (Channel::Ui, 0.8),
        ] {
            v.levels[ch.index()] = level;
        }
        v
    }
}

impl Volumes {
    pub fn get(&self, ch: Channel) -> f32 {
        self.levels[ch.index()]
    }

    /// Règle un canal, borné à 0..1.
    pub fn set(&mut self, ch: Channel, v: f32) {
        self.levels[ch.index()] = v.clamp(0.0, 1.0);
    }

    /// Gain effectif d'un canal : le sien multiplié par le master (nul si coupé).
    pub fn gain(&self, ch: Channel) -> f32 {
        let master = if self.muted {
            0.0
        } else {
            self.get(Channel::Master)
        };
        match ch {
            Channel::Master => master,
            other => master * self.get(other),
        }
    }
}

/// Gain spatial : 1 près de la caméra, puis décroissance linéaire jusqu'à un plancher de 0,1.
pub fn spatial_gain(pos: Option<Vec2>, listener: Vec2) -> f32 {
    let Some(p) = pos else { return 1.0 };
    let d = (p.x - listener.x).hypot(p.y - listener.y);
    if d <= 800.0 {
        return 1.0;
    }
    (1.0 - (d - 800.0) / 3200.0).max(0.1) as f32
}

/// Limite de polyphonie du canal des effets : chaque effet admis occupe une place pendant
/// [`SFX_SLOT_SECONDS`], au-delà de [`MAX_SFX`] places les nouveaux effets sont ignorés.
#[derive(Debug, Default)]
pub struct SfxGate {
    /// Instants de libération des places occupées, croissants.
    releases: VecDeque<f64>,
}

impl SfxGate {
    /// Demande une place à l'instant `now` (secondes) ; `false` si la polyphonie est atteinte.
    pub fn admit(&mut self, now: f64) -> bool {
        while self.releases.front().is_some_and(|&t| t <= now) {
            self.releases.pop_front();
        }
        if self.releases.len() >= MAX_SFX {
            return false;
        }
        self.releases.push_back(now + SFX_SLOT_SECONDS);
        true
    }

    #[cfg(test)]
    pub fn active(&self) -> usize {
        self.releases.len()
    }
}

/// Un effet : quelques voix jouées ensemble sur un canal, avec un gain commun (spatial × volume).
#[derive(Debug, Clone)]
pub struct Sfx {
    pub channel: Channel,
    pub gain: f32,
    pub voices: Vec<Voice>,
}

impl Sfx {
    pub fn new(channel: Channel, gain: f32) -> Self {
        Self {
            channel,
            gain,
            voices: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn with(mut self, voice: Voice) -> Self {
        self.voices.push(voice);
        self
    }

    fn done(&self) -> bool {
        self.voices.iter().all(Voice::done)
    }

    fn next(&mut self) -> f32 {
        let s: f64 = self.voices.iter_mut().map(Voice::next).sum();
        s as f32 * self.gain
    }
}

/// Effets actifs au-delà desquels la liste réalloue : de quoi couvrir la polyphonie et les autres canaux.
const ACTIVE_CAPACITY: usize = 128;

/// Bus de musique au plus (le web en crée deux : la musique et les jingles d'élimination).
pub const MAX_BUSES: usize = 4;

/// Mélangeur du fil audio : additionne les bus de musique et les effets actifs, applique les volumes de
/// canal, sort en stéréo.
#[derive(Debug)]
pub struct Mixer {
    active: Vec<Sfx>,
    buses: Vec<(Channel, Box<MusicBus>)>,
    volumes: Volumes,
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            active: Vec::with_capacity(ACTIVE_CAPACITY),
            buses: Vec::with_capacity(MAX_BUSES),
            volumes: Volumes::default(),
        }
    }
}

impl Mixer {
    pub fn play(&mut self, sfx: Sfx) {
        self.active.push(sfx);
    }

    /// Branche un bus de musique sur un canal ; au-delà de [`MAX_BUSES`], il est ignoré.
    pub fn add_bus(&mut self, channel: Channel, bus: impl Into<Box<MusicBus>>) {
        if self.buses.len() < MAX_BUSES {
            self.buses.push((channel, bus.into()));
        }
    }

    /// Transmet une commande au bus `id` (ignorée s'il n'existe pas).
    pub fn bus_command(&mut self, id: u32, cmd: BusCommand) {
        if let Some((_, bus)) = self.buses.iter_mut().find(|(_, b)| b.id() == id) {
            bus.command(cmd);
        }
    }

    pub fn set_volumes(&mut self, v: Volumes) {
        self.volumes = v;
    }

    #[cfg(test)]
    pub fn active(&self) -> usize {
        self.active.len()
    }

    /// Remplit `out` d'échantillons stéréo entrelacés (gauche, droite), puis retire les effets terminés.
    pub fn render(&mut self, out: &mut [f32]) {
        let mut gains = [0.0; 5];
        for ch in AUDIO_CHANNELS {
            gains[ch.index()] = self.volumes.gain(ch);
        }
        out.fill(0.0);
        for (ch, bus) in &mut self.buses {
            bus.render_add(out, gains[ch.index()]);
        }
        for frame in out.as_chunks_mut::<2>().0 {
            let mut s = 0.0;
            for sfx in &mut self.active {
                s += sfx.next() * gains[sfx.channel.index()];
            }
            // Comme la sortie WebAudio : écrêtage à ±1, sans compression.
            frame[0] = (frame[0] + s).clamp(-1.0, 1.0);
            frame[1] = (frame[1] + s).clamp(-1.0, 1.0);
        }
        self.active.retain(|sfx| !sfx.done());
    }
}

#[cfg(test)]
mod tests {
    use super::super::synth::{NoiseOpts, SAMPLE_RATE, ToneOpts, noise, tone};
    use super::*;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }

    #[test]
    fn volumes_par_defaut_et_bornes() {
        let mut vol = Volumes::default();
        assert_eq!(vol.get(Channel::Music), 0.5);
        vol.set(Channel::Sfx, 3.0);
        assert_eq!(vol.get(Channel::Sfx), 1.0);
        assert!(
            (vol.gain(Channel::Sfx) - 0.8).abs() < 1e-6,
            "master × canal"
        );
        vol.muted = true;
        assert_eq!(vol.gain(Channel::Ui), 0.0);
    }

    #[test]
    fn attenuation_selon_la_distance_a_la_camera() {
        let cam = v(6000.0, 6000.0);
        assert_eq!(spatial_gain(None, cam), 1.0);
        assert_eq!(spatial_gain(Some(v(6500.0, 6000.0)), cam), 1.0);
        assert!((spatial_gain(Some(v(8400.0, 6000.0)), cam) - 0.5).abs() < 1e-6);
        assert_eq!(spatial_gain(Some(v(0.0, 0.0)), cam), 0.1);
    }

    #[test]
    fn polyphonie_limitee_a_24_effets_pendant_2_5_s() {
        let mut gate = SfxGate::default();
        for _ in 0..MAX_SFX {
            assert!(gate.admit(0.0));
        }
        assert!(!gate.admit(1.0), "25e effet ignoré");
        assert!(gate.admit(2.5), "places libérées après 2,5 s");
        assert_eq!(gate.active(), 1);
    }

    #[test]
    fn le_mixeur_applique_les_canaux_et_retire_les_effets_finis() {
        let render = |vol: Volumes| {
            let mut m = Mixer::default();
            m.set_volumes(vol);
            m.play(Sfx::new(Channel::Ui, 1.0).with(tone(ToneOpts::default())));
            m.play(Sfx::new(Channel::Sfx, 0.5).with(noise(NoiseOpts::default(), 3)));
            let mut out = vec![0.0; SAMPLE_RATE as usize]; // 0,5 s stéréo
            m.render(&mut out);
            (out, m.active())
        };
        let (out, left) = render(Volumes::default());
        assert!(out.iter().any(|s| s.abs() > 0.01));
        assert!(
            out.as_chunks::<2>().0.iter().all(|f| f[0] == f[1]),
            "mono centré"
        );
        assert_eq!(left, 0, "les deux effets sont terminés");
        let muted = Volumes {
            muted: true,
            ..Volumes::default()
        };
        assert!(render(muted).0.iter().all(|&s| s == 0.0));
    }
}
