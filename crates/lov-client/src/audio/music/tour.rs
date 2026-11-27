//! Visite des thèmes pour la démo : `LOV_AUDIOLAB=themes`.
//!
//! Le vrai séquenceur ([`Music`]) joue chaque thème à tour de rôle, avec ses fondus enchaînés : le menu
//! (une boucle), le draft (une boucle), la partie au calme puis en plein combat, et enfin la victoire et la
//! défaite. Puis la visite recommence. Le nom de chaque étape s'écrit dans la console.

use bevy::prelude::*;

use super::super::AudioManager;
use super::super::mixer::Channel;
use super::sequencer::{Music, Want};
use super::themes::ThemeId;
use super::{BusCommand, MusicBusHandle};

/// Une étape : nom, durée (secondes réelles), écran demandé, intensité imposée.
type Stop = (&'static str, f64, Want, Option<f64>);

const STOPS: &[Stop] = &[
    (
        "menu : « La Faille des Vibes »",
        26.0,
        Want::Theme(ThemeId::Menu),
        None,
    ),
    (
        "draft : « Le Choix »",
        19.0,
        Want::Theme(ThemeId::Draft),
        None,
    ),
    (
        "partie au calme : « Sous la canopée »",
        30.0,
        Want::Theme(ThemeId::Game),
        Some(0.0),
    ),
    (
        "partie en plein combat",
        30.0,
        Want::Theme(ThemeId::Game),
        Some(1.0),
    ),
    ("fin : victoire", 8.0, Want::End, None),
    ("fin : défaite", 8.0, Want::End, None),
];

#[derive(Default)]
pub struct Tour {
    music: Music,
    bus: Option<MusicBusHandle>,
    index: usize,
    due: f64,
}

/// Système de la visite (ajouté par la démo à la place de la galerie).
pub fn tour(time: Res<Time<Real>>, mut audio: ResMut<AudioManager>, mut tour: Local<Tour>) {
    let Tour {
        music,
        bus,
        index,
        due,
    } = &mut *tour;
    let bus = bus.get_or_insert_with(|| audio.music_bus(Channel::Music));
    let real = time.elapsed_secs_f64();
    let now = bus.current_time();
    let mut out = |cmd: BusCommand| audio.bus(bus, cmd);
    if real >= *due {
        let (name, seconds, want, intensity) = STOPS[*index % STOPS.len()];
        let defeat = *index % STOPS.len() == STOPS.len() - 1;
        *index += 1;
        *due = real + seconds;
        info!("audiolab : thème — {name}");
        if defeat {
            // déjà en fin de partie : seul le second jingle part
            music.schedule_stinger(false, real);
        } else {
            music.follow(want, true, 0.0, real, now, &mut out);
        }
        if let Some(v) = intensity {
            music.intensity.value = v;
            music.intensity.floor = v;
        }
    }
    music.tick(real, time.delta_secs_f64(), now, &mut out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_visite_passe_par_chaque_theme_puis_les_deux_fins() {
        let themes: Vec<Want> = STOPS.iter().map(|s| s.2).collect();
        for id in [ThemeId::Menu, ThemeId::Draft, ThemeId::Game] {
            assert!(themes.contains(&Want::Theme(id)));
        }
        assert_eq!(&themes[themes.len() - 2..], &[Want::End, Want::End]);
        assert!(STOPS.iter().all(|s| s.1 > 5.0));
    }
}
