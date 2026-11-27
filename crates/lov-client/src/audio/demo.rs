//! Démo du moteur audio, sans serveur : `LOV_AUDIOLAB=1 cargo run -p lov-client`.
//!
//! Rejoue en boucle une galerie de sons, un toutes les 1,2 s, et écrit son nom dans la console : les
//! quatre formes d'onde, glissements, filtres de bruit, une recette d'ult portée du client web, la
//! spatialisation (le même coup de plus en plus loin) et une rafale qui bute sur la polyphonie ; puis la
//! galerie de musique : chaque instrument sur un bus du canal Musique, puis les jingles d'élimination sur un
//! bus du canal Voix. Enfin, une séquence d'événements de jeu factices fait entendre la table événement → son.

use bevy::prelude::*;
use lov_shared::types::Vec2 as GameVec2;

use super::mixer::Channel;
use super::music::instruments::*;
use super::music::stings::{StingSide, first_blood_sting, kill_sting};
use super::music::{BusCommand, MusicBusHandle, Score};
use super::sounds::lab::{EventLab, LAB};
use super::synth::{FilterKind, NoiseOpts, ToneOpts, Wave};
use super::{AudioManager, Recipe, SfxOpts};

/// `LOV_AUDIOLAB` défini (et non vide) : lancer la démo.
pub fn enabled() -> bool {
    std::env::var("LOV_AUDIOLAB").is_ok_and(|v| !v.is_empty())
}

pub struct AudioLabPlugin;

impl Plugin for AudioLabPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, replay);
    }
}

const PERIOD: f32 = 1.2;

type Entry = (&'static str, fn(&mut AudioManager));

fn at(listener: GameVec2, dx: f64) -> SfxOpts {
    SfxOpts {
        pos: Some(GameVec2 {
            x: listener.x + dx,
            y: listener.y,
        }),
        ..SfxOpts::default()
    }
}

fn thud(r: &mut Recipe) {
    r.noise(NoiseOpts {
        dur: 0.5,
        vol: 0.3,
        filter: Some(800.0),
        slide_filter: Some(120.0),
        ..NoiseOpts::default()
    })
    .tone(ToneOpts {
        wave: Wave::Sawtooth,
        freq: 160.0,
        slide: 50.0 / 160.0,
        dur: 0.5,
        vol: 0.25,
        filter: Some(600.0),
        ..ToneOpts::default()
    });
}

const GALLERY: &[Entry] = &[
    ("ondes : sinus, carré, dent de scie, triangle", |a| {
        a.sfx(SfxOpts::default(), |r| {
            for (i, wave) in [Wave::Sine, Wave::Square, Wave::Sawtooth, Wave::Triangle]
                .into_iter()
                .enumerate()
            {
                r.tone(ToneOpts {
                    wave,
                    freq: 330.0,
                    dur: 0.22,
                    delay: i as f64 * 0.25,
                    ..ToneOpts::default()
                });
            }
        });
    }),
    ("glissements montant et descendant", |a| {
        a.sfx(SfxOpts::default(), |r| {
            r.tone(ToneOpts {
                freq: 300.0,
                slide: 3.0,
                dur: 0.4,
                ..ToneOpts::default()
            })
            .tone(ToneOpts {
                wave: Wave::Square,
                freq: 660.0,
                slide: 0.5,
                dur: 0.4,
                delay: 0.45,
                vol: 0.12,
                ..ToneOpts::default()
            });
        });
    }),
    (
        "bruits : passe-bas glissant, passe-bande, passe-haut",
        |a| {
            a.sfx(SfxOpts::default(), |r| {
                for (i, (kind, f, to)) in [
                    (FilterKind::Lowpass, 3000.0, Some(300.0)),
                    (FilterKind::Bandpass, 1800.0, None),
                    (FilterKind::Highpass, 3500.0, None),
                ]
                .into_iter()
                .enumerate()
                {
                    r.noise(NoiseOpts {
                        dur: 0.3,
                        vol: 0.3,
                        delay: i as f64 * 0.33,
                        filter: Some(f),
                        slide_filter: to,
                        kind,
                    });
                }
            });
        },
    ),
    ("montée de niveau (interface)", |a| {
        let opts = SfxOpts {
            channel: Channel::Ui,
            ..SfxOpts::default()
        };
        a.sfx(opts, |r| {
            for (i, (f, d)) in [(523.0, 0.15), (659.0, 0.15), (784.0, 0.3)]
                .into_iter()
                .enumerate()
            {
                r.tone(ToneOpts {
                    freq: f,
                    dur: d,
                    vol: 0.22,
                    delay: i as f64 * 0.1,
                    ..ToneOpts::default()
                });
            }
        });
    }),
    ("ult de Grondak (recette du client web)", |a| {
        a.sfx(SfxOpts::default(), |r| {
            let n = |dur, vol, filter, slide: Option<f64>, delay| NoiseOpts {
                dur,
                vol,
                filter: Some(filter),
                slide_filter: slide,
                delay,
                ..NoiseOpts::default()
            };
            r.noise(n(0.75, 0.2, 220.0, Some(90.0), 0.0))
                .noise(n(1.1, 0.5, 700.0, Some(70.0), 0.75))
                .tone(ToneOpts {
                    freq: 48.0,
                    slide: 30.0,
                    dur: 1.2,
                    vol: 0.5,
                    delay: 0.75,
                    ..ToneOpts::default()
                });
            for i in 0..5 {
                let i = f64::from(i);
                r.noise(n(0.08, 0.15, 2000.0 + i * 500.0, None, 0.75 + i * 0.09));
            }
        });
    }),
    ("spatialisation : 0, 1 600, 2 400 puis 4 000 unités", |a| {
        let l = a.listener;
        for (i, dx) in [0.0, 1600.0, 2400.0, 4000.0].into_iter().enumerate() {
            a.sfx(at(l, dx), |r| {
                r.tone(ToneOpts {
                    wave: Wave::Square,
                    freq: 880.0,
                    dur: 0.12,
                    vol: 0.25,
                    delay: i as f64 * 0.28,
                    ..ToneOpts::default()
                });
            });
        }
    }),
    ("rafale de 30 morts : 24 passent (polyphonie)", |a| {
        let played = (0..30).filter(|_| a.sfx(SfxOpts::default(), thud)).count();
        info!("audiolab : {played} effets joués sur 30");
    }),
];

/// Une entrée de la galerie de musique : nom, durée avant la suivante, phrase jouée à l'instant `t`.
/// Les noms en « jingle » passent par un bus du canal Voix au gain global 1, comme dans `sounds.ts`.
type MusicEntry = (&'static str, f32, fn(&mut Score, f64));

const MUSIC: &[MusicEntry] = &[
    ("nappe (pad)", 2.4, |s, t| {
        pad(s, &[57, 60, 64], t, 2.0, 0.05, 1.0)
    }),
    ("chœur", 2.4, |s, t| choir(s, &[69, 76], t, 2.0, 0.04)),
    ("cordes en ostinato", 1.8, |s, t| {
        for (i, n) in [64, 64, 67, 64, 71, 69, 67, 64].into_iter().enumerate() {
            strings(s, n, t + i as f64 * 0.18, 0.16, 0.035);
        }
    }),
    ("cuivres", 1.5, |s, t| {
        brass(s, &[60, 64, 67], t, 0.5, 0.045);
        brass(s, &[62, 65, 69], t + 0.6, 0.6, 0.045);
    }),
    ("basse", 1.8, |s, t| {
        for (i, n) in [33, 33, 36, 31].into_iter().enumerate() {
            bass(s, n, t + i as f64 * 0.4, 0.35, 0.12);
        }
    }),
    ("cloches", 2.8, |s, t| {
        for (i, n) in [76, 79, 83, 88].into_iter().enumerate() {
            bell(s, n, t + i as f64 * 0.25, 0.05, 2.4);
        }
    }),
    ("flûte", 2.0, |s, t| {
        flute(s, 76, t, 0.5, 0.05);
        flute(s, 79, t + 0.55, 1.0, 0.05);
    }),
    ("arpège pincé", 1.6, |s, t| {
        for (i, n) in [60, 64, 67, 72, 76, 72, 67, 64].into_iter().enumerate() {
            pluck(s, n, t + i as f64 * 0.12, 0.04);
        }
    }),
    ("taiko et caisse claire", 1.8, |s, t| {
        for i in 0..4 {
            let at = t + f64::from(i) * 0.3;
            taiko(s, at, 0.32, 1.0);
            snare(s, at + 0.15, 0.12);
        }
    }),
    ("montée de cymbale puis crash", 2.6, |s, t| {
        swell(s, t, 0.8, 0.06);
        crash(s, t + 0.8, 0.06);
    }),
    ("nappe douce et kalimba", 4.0, |s, t| {
        soft_pad(s, &[57, 64], t, 3.0, 0.03, 1.0);
        for (i, n) in [69, 72, 76, 74].into_iter().enumerate() {
            kalimba(s, n, t + 0.4 + i as f64 * 0.45, 0.03);
        }
    }),
    ("jingle : premier sang (allié)", 3.6, |s, t| {
        first_blood_sting(s, t, StingSide::Ally)
    }),
    ("jingle : premier sang (ennemi)", 3.6, |s, t| {
        first_blood_sting(s, t, StingSide::Enemy)
    }),
    ("jingle : kill simple (allié)", 1.8, |s, t| {
        kill_sting(s, t, 0, StingSide::Ally)
    }),
    ("jingle : double kill (soi)", 2.0, |s, t| {
        kill_sting(s, t, 2, StingSide::Own)
    }),
    ("jingle : triple kill (allié)", 2.6, |s, t| {
        kill_sting(s, t, 4, StingSide::Ally)
    }),
    ("jingle : triple kill (ennemi)", 2.6, |s, t| {
        kill_sting(s, t, 4, StingSide::Enemy)
    }),
    ("jingle : quadra kill (soi)", 3.0, |s, t| {
        kill_sting(s, t, 6, StingSide::Own)
    }),
    ("jingle : penta kill (soi)", 3.6, |s, t| {
        kill_sting(s, t, 8, StingSide::Own)
    }),
    ("jingle : série légendaire (soi)", 4.0, |s, t| {
        kill_sting(s, t, 9, StingSide::Own)
    }),
];

#[derive(Default)]
struct Lab {
    due: f32,
    index: usize,
    music: Option<MusicBusHandle>,
    stings: Option<MusicBusHandle>,
    events: EventLab,
}

fn replay(time: Res<Time>, mut audio: ResMut<AudioManager>, mut lab: Local<Lab>) {
    if time.elapsed_secs() < lab.due {
        return;
    }
    let i = lab.index % (GALLERY.len() + MUSIC.len() + LAB.len());
    lab.index += 1;
    if let Some(&(name, period, play)) = i
        .checked_sub(GALLERY.len() + MUSIC.len())
        .and_then(|e| LAB.get(e))
    {
        lab.due = time.elapsed_secs() + period;
        info!("audiolab : {name}");
        play(&mut audio, &mut lab.events);
        return;
    }
    let Some(&(name, period, play)) = i.checked_sub(GALLERY.len()).and_then(|m| MUSIC.get(m))
    else {
        // la rafale occupe la polyphonie 2,5 s : on laisse l'entrée suivante démarrer après
        let burst = i == GALLERY.len() - 1;
        lab.due = time.elapsed_secs() + if burst { 3.0 } else { PERIOD };
        let (name, play) = GALLERY[i];
        info!("audiolab : {name}");
        play(&mut audio);
        return;
    };
    lab.due = time.elapsed_secs() + period;
    info!("audiolab : musique — {name}");
    let jingle = name.starts_with("jingle");
    let slot = if jingle {
        &mut lab.stings
    } else {
        &mut lab.music
    };
    let bus = match slot {
        Some(bus) => bus.clone(),
        None => {
            let bus = audio.music_bus(if jingle {
                Channel::Voice
            } else {
                Channel::Music
            });
            let open = if jingle {
                BusCommand::SetMaster(1.0)
            } else {
                BusCommand::Fade {
                    to: 0.95,
                    seconds: 0.3,
                }
            };
            audio.bus(&bus, open);
            *slot = Some(bus.clone());
            bus
        }
    };
    let mut score = Score::new();
    play(&mut score, bus.current_time() + 0.05);
    audio.bus(&bus, BusCommand::Play(score));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_entree_de_la_galerie_de_musique_programme_des_notes() {
        for &(name, period, play) in MUSIC {
            let mut s = Score::new();
            play(&mut s, 1.0);
            assert!(!s.is_empty(), "{name}");
            assert!(s.end() > 1.0 && period > 1.0, "{name}");
        }
    }
}
