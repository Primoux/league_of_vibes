//! Démo du moteur audio, sans serveur : `LOV_AUDIOLAB=1 cargo run -p lov-client`.
//!
//! Rejoue en boucle une galerie de sons, un toutes les 1,2 s, et écrit son nom dans la console : les
//! quatre formes d'onde, glissements, filtres de bruit, une recette d'ult portée du client web, la
//! spatialisation (le même coup de plus en plus loin) et une rafale qui bute sur la polyphonie.

use bevy::prelude::*;
use lov_shared::types::Vec2 as GameVec2;

use super::mixer::Channel;
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

fn replay(time: Res<Time>, mut audio: ResMut<AudioManager>, mut next: Local<(f32, usize)>) {
    let (due, index) = &mut *next;
    if time.elapsed_secs() < *due {
        return;
    }
    // la rafale occupe la polyphonie 2,5 s : on laisse l'entrée suivante démarrer après
    let burst = *index % GALLERY.len() == GALLERY.len() - 1;
    *due = time.elapsed_secs() + if burst { 3.0 } else { PERIOD };
    let (name, play) = GALLERY[*index % GALLERY.len()];
    info!("audiolab : {name}");
    play(&mut audio);
    *index += 1;
}
