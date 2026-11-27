//! Audio du client natif : effets et musique entièrement synthétisés, aucun asset externe.
//!
//! Portage de `packages/client/src/audio/` (issue #118), en plusieurs PR :
//! - le moteur ([`synth`]), le mixage par canaux ([`mixer`]), les séries de kills ([`kill_streaks`]) et
//!   la sortie vers la carte son ;
//! - le moteur de musique ([`music`]) : bus, réverbération, instruments, jingles d'élimination, puis les
//!   thèmes et le séquenceur qui suit l'écran ([`music::sequencer`]) ;
//! - ensuite : la table événement de jeu → son (`sounds.ts`).
//!
//! Le client web pilote un `AudioContext` ; ici le jeu envoie des [`Sfx`] tout faits au fil audio par un
//! canal, et un [`Mixer`] les rend dans une source Bevy sans fin ([`SynthStream`]). Rien n'est alloué ni
//! verrouillé par échantillon sur le fil audio.

pub mod demo;
// Les séries de kills, les volumes et une partie de l'API de synthèse servent à la table événement → son
// et à la musique, portées dans les PR suivantes de #118 ; jusque-là, seuls les tests et la démo les appellent.
#[allow(dead_code)]
pub mod kill_streaks;
#[allow(dead_code)]
pub mod mixer;
// Le bus et les jingles servent à la table événement → son et aux thèmes (PR suivantes de #118) ; d'ici là,
// seuls la démo et les tests en appellent une partie.
#[allow(dead_code)]
pub mod music;
#[allow(dead_code)]
pub mod synth;

use std::num::NonZero;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source};
use bevy::prelude::*;
use lov_shared::types::{EntityId, Vec2 as GameVec2};

use crate::net::GameState;
use crate::render::camera::CameraRig;
use mixer::{Channel, Mixer, Sfx, SfxGate, Volumes, spatial_gain};
use music::{BusCommand, MusicBus, MusicBusHandle};
use synth::{NoiseOpts, SAMPLE_RATE, ToneOpts, Voice};

/// Messages en attente au plus entre le jeu et le fil audio, qui les lit tous les 256 échantillons.
const COMMAND_QUEUE: usize = 256;

/// Message du jeu vers le fil audio.
enum Command {
    Play(Sfx),
    Volumes(Volumes),
    /// Bus de musique construit côté jeu (ses allocations n'ont pas lieu sur le fil audio).
    AddBus(Channel, Box<MusicBus>),
    Bus(u32, BusCommand),
}

/// Options d'un effet (`SfxOpts`) : position pour la spatialisation, volume, canal (effets par défaut).
#[derive(Debug, Clone, Copy)]
pub struct SfxOpts {
    pub pos: Option<GameVec2>,
    pub vol: f32,
    pub channel: Channel,
}

impl Default for SfxOpts {
    fn default() -> Self {
        Self {
            pos: None,
            vol: 1.0,
            channel: Channel::Sfx,
        }
    }
}

/// Les voix d'un effet en construction : l'équivalent du `(ctx, dest) => { tone(…); noise(…) }` web.
pub struct Recipe<'a> {
    voices: Vec<Voice>,
    seed: &'a mut u32,
}

// `random` sert aux recettes à variations (cristaux de l'ult d'Ilyra), portées dans la PR suivante de #118.
#[allow(dead_code)]
impl Recipe<'_> {
    pub fn tone(&mut self, o: ToneOpts) -> &mut Self {
        self.voices.push(synth::tone(o));
        self
    }

    pub fn noise(&mut self, o: NoiseOpts) -> &mut Self {
        // Chaque bruit a sa graine : deux souffles simultanés ne sont pas identiques.
        *self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        self.voices.push(synth::noise(o, *self.seed));
        self
    }

    /// Valeur dans `[0, 1)` tirée du même générateur semé, pour les variations des recettes.
    pub fn random(&mut self) -> f64 {
        *self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        f64::from(*self.seed) / f64::from(u32::MAX)
    }
}

/// Gestionnaire audio côté jeu (`AudioManager` du client web) : volumes, écouteur, polyphonie.
#[derive(Resource)]
pub struct AudioManager {
    tx: SyncSender<Command>,
    volumes: Volumes,
    gate: SfxGate,
    /// Position de l'écouteur (centre de la caméra), en unités de jeu.
    listener: GameVec2,
    /// Horloge du gestionnaire (secondes), avancée chaque image : sert à la polyphonie.
    now: f64,
    seed: u32,
    /// Identifiant du prochain bus de musique.
    next_bus: u32,
}

// Volumes et positions d'entités servent aux paramètres et à la table événement → son (PR suivantes de #118).
#[allow(dead_code)]
impl AudioManager {
    fn new(tx: SyncSender<Command>) -> Self {
        Self {
            tx,
            volumes: Volumes::default(),
            gate: SfxGate::default(),
            listener: GameVec2 {
                x: 6000.0,
                y: 6000.0,
            },
            now: 0.0,
            seed: 0x5eed,
            next_bus: 1,
        }
    }

    /// Gestionnaire sans fil audio, pour les tests : renvoie aussi ce que le fil audio recevrait.
    #[cfg(test)]
    fn for_tests() -> (Self, Receiver<Command>) {
        let (tx, rx) = sync_channel(COMMAND_QUEUE);
        (Self::new(tx), rx)
    }

    pub fn set_volume(&mut self, ch: Channel, v: f32) {
        self.volumes.set(ch, v);
        self.send(Command::Volumes(self.volumes));
    }

    pub fn volume(&self, ch: Channel) -> f32 {
        self.volumes.get(ch)
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.volumes.muted = muted;
        self.send(Command::Volumes(self.volumes));
    }

    pub fn muted(&self) -> bool {
        self.volumes.muted
    }

    pub fn set_listener(&mut self, pos: GameVec2) {
        self.listener = pos;
    }

    /// Joue un effet : `build` ajoute ses voix. Sur le canal des effets, ignoré au-delà de la polyphonie.
    /// Renvoie `true` si l'effet est parti.
    pub fn sfx(&mut self, opts: SfxOpts, build: impl FnOnce(&mut Recipe)) -> bool {
        if opts.channel == Channel::Sfx && !self.gate.admit(self.now) {
            return false;
        }
        let mut recipe = Recipe {
            voices: Vec::new(),
            seed: &mut self.seed,
        };
        build(&mut recipe);
        let gain = spatial_gain(opts.pos, self.listener) * opts.vol;
        let mut sfx = Sfx::new(opts.channel, gain);
        sfx.voices = recipe.voices;
        self.send(Command::Play(sfx));
        true
    }

    /// Crée un bus de musique sur `channel` (`new MusicBus(ctx, dest)` dans un `sfx` du web). Gain global
    /// à 0 : le jeu l'ouvre par [`BusCommand::Fade`] ou `SetMaster`.
    pub fn music_bus(&mut self, channel: Channel) -> MusicBusHandle {
        let (bus, handle) = MusicBus::new(self.next_bus);
        self.next_bus += 1;
        self.send(Command::AddBus(channel, Box::new(bus)));
        handle
    }

    /// Envoie une commande (phrase, fondu, délai) à un bus de musique.
    pub fn bus(&mut self, bus: &MusicBusHandle, cmd: BusCommand) {
        self.send(Command::Bus(bus.id(), cmd));
    }

    /// Position d'une entité dans l'état courant, pour la spatialisation.
    pub fn entity_pos(state: &GameState, id: Option<EntityId>) -> Option<GameVec2> {
        let e = state.entities().get(&id?)?;
        Some(GameVec2 { x: e.x, y: e.y })
    }

    fn send(&self, cmd: Command) {
        // File pleine ou fil audio absent (pas de carte son, personne ne lit) : le son est perdu, le jeu
        // continue, et la file ne grossit pas indéfiniment.
        let _ = self.tx.try_send(cmd);
    }
}

/// Source Bevy sans fin qui rend le [`Mixer`]. Le récepteur est pris par le premier décodeur créé.
#[derive(Asset, TypePath)]
pub struct SynthStream {
    rx: Mutex<Option<Receiver<Command>>>,
}

impl Decodable for SynthStream {
    type Decoder = Stream;

    fn decoder(&self) -> Stream {
        let rx = self.rx.lock().ok().and_then(|mut slot| slot.take());
        Stream::new(rx)
    }
}

/// Trames rendues d'un coup ; les commandes du jeu sont lues entre deux blocs.
const BLOCK_FRAMES: usize = 256;

/// Décodeur de [`SynthStream`] : échantillons stéréo entrelacés à [`SAMPLE_RATE`].
pub struct Stream {
    mixer: Mixer,
    rx: Option<Receiver<Command>>,
    buf: Box<[f32; BLOCK_FRAMES * 2]>,
    pos: usize,
}

impl Stream {
    fn new(rx: Option<Receiver<Command>>) -> Self {
        Self {
            mixer: Mixer::default(),
            rx,
            buf: Box::new([0.0; BLOCK_FRAMES * 2]),
            pos: BLOCK_FRAMES * 2,
        }
    }

    fn refill(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    Command::Play(sfx) => self.mixer.play(sfx),
                    Command::Volumes(v) => self.mixer.set_volumes(v),
                    Command::AddBus(ch, bus) => self.mixer.add_bus(ch, bus),
                    Command::Bus(id, cmd) => self.mixer.bus_command(id, cmd),
                }
            }
        }
        self.mixer.render(&mut self.buf[..]);
        self.pos = 0;
    }
}

impl Iterator for Stream {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pos >= self.buf.len() {
            self.refill();
        }
        let s = self.buf[self.pos];
        self.pos += 1;
        Some(s)
    }
}

impl Source for Stream {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZero::<u16>::MIN.saturating_add(1)
    }

    fn sample_rate(&self) -> SampleRate {
        NonZero::new(SAMPLE_RATE).unwrap_or(NonZero::<u32>::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Flux en attente d'être placé dans les assets au démarrage.
#[derive(Resource)]
struct PendingStream(Option<SynthStream>);

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = sync_channel(COMMAND_QUEUE);
        app.add_audio_source::<SynthStream>()
            .insert_resource(AudioManager::new(tx))
            .insert_resource(PendingStream(Some(SynthStream {
                rx: Mutex::new(Some(rx)),
            })))
            .add_plugins(music::sequencer::MusicPlugin)
            .add_systems(Startup, start_stream)
            .add_systems(First, (tick_clock, follow_camera));
    }
}

fn start_stream(
    mut commands: Commands,
    mut pending: ResMut<PendingStream>,
    mut streams: ResMut<Assets<SynthStream>>,
) {
    if let Some(stream) = pending.0.take() {
        commands.spawn(AudioPlayer(streams.add(stream)));
    }
    commands.remove_resource::<PendingStream>();
}

fn tick_clock(time: Res<Time<Real>>, mut audio: ResMut<AudioManager>) {
    audio.now = time.elapsed_secs_f64();
}

/// L'écouteur suit le centre de la caméra (`setListener` du client web).
fn follow_camera(rig: Option<Res<CameraRig>>, mut audio: ResMut<AudioManager>) {
    if let Some(rig) = rig {
        let p = crate::render::to_game(rig.focus);
        audio.set_listener(GameVec2 {
            x: f64::from(p.x),
            y: f64::from(p.y),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_flux_rend_les_effets_recus_en_stereo() {
        let (mut am, rx) = AudioManager::for_tests();
        assert!(am.sfx(SfxOpts::default(), |r| {
            r.tone(ToneOpts::default()).noise(NoiseOpts::default());
        }));
        let mut s = Stream::new(Some(rx));
        assert_eq!(s.channels().get(), 2);
        assert_eq!(s.sample_rate().get(), SAMPLE_RATE);
        let samples: Vec<f32> = s.by_ref().take(SAMPLE_RATE as usize / 5).collect();
        assert!(
            samples.iter().any(|x| x.abs() > 0.01),
            "l'effet est audible"
        );
    }

    #[test]
    fn la_polyphonie_et_le_volume_passent_par_le_gestionnaire() {
        let (mut am, rx) = AudioManager::for_tests();
        let played = (0..30)
            .filter(|_| {
                am.sfx(SfxOpts::default(), |r| {
                    r.tone(ToneOpts::default());
                })
            })
            .count();
        assert_eq!(played, mixer::MAX_SFX);
        // l'interface n'est pas limitée
        assert!(am.sfx(
            SfxOpts {
                channel: Channel::Ui,
                ..SfxOpts::default()
            },
            |_| {}
        ));
        am.now = 3.0;
        assert!(am.sfx(SfxOpts::default(), |_| {}), "places libérées");
        am.set_muted(true);
        assert!(am.muted());
        let mut s = Stream::new(Some(rx));
        assert!(s.by_ref().take(4800).all(|x| x == 0.0), "coupé");
    }

    #[test]
    fn sans_fil_audio_la_file_reste_bornee_et_ne_bloque_pas() {
        let (mut am, rx) = AudioManager::for_tests();
        let ui = SfxOpts {
            channel: Channel::Ui,
            ..SfxOpts::default()
        };
        for _ in 0..COMMAND_QUEUE * 4 {
            am.sfx(ui, |r| {
                r.tone(ToneOpts::default());
            });
        }
        assert_eq!(rx.try_iter().count(), COMMAND_QUEUE);
    }

    #[test]
    fn le_gestionnaire_cree_et_pilote_un_bus_de_musique() {
        let (mut am, rx) = AudioManager::for_tests();
        let bus = am.music_bus(Channel::Voice);
        let mut score = music::Score::new();
        music::stings::kill_sting(&mut score, 0.05, 0, music::stings::StingSide::Ally);
        am.bus(&bus, BusCommand::SetMaster(1.0));
        am.bus(&bus, BusCommand::Play(score));
        let mut s = Stream::new(Some(rx));
        let samples: Vec<f32> = s.by_ref().take(SAMPLE_RATE as usize).collect();
        assert!(samples.iter().any(|x| x.abs() > 0.01), "jingle audible");
        assert!(bus.current_time() > 0.49, "horloge du bus publiée");
    }

    #[test]
    fn la_distance_attenue_le_gain_de_l_effet() {
        let (mut am, rx) = AudioManager::for_tests();
        am.set_listener(GameVec2 { x: 0.0, y: 0.0 });
        am.sfx(
            SfxOpts {
                pos: Some(GameVec2 { x: 2400.0, y: 0.0 }),
                ..SfxOpts::default()
            },
            |_| {},
        );
        match rx.try_recv() {
            Ok(Command::Play(sfx)) => assert!((sfx.gain - 0.5).abs() < 1e-6),
            _ => panic!("un effet était attendu"),
        }
    }
}
