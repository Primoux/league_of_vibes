//! Table de correspondance événement de jeu → sons synthétisés.
//!
//! Portage de `packages/client/src/audio/sounds.ts`. Chaque `fxKey` des sorts a une recette procédurale
//! (dérivée du champion et du slot, [`recipes`]), les sorts d'invocateur ont leur propre table, et tout
//! événement a un son par type. Les éliminations entre champions jouent un jingle ([`kills`]).
//!
//! Branchement : comme `fxBus.subscribe((e) => audio.handleEvent(e))` du web, un système lit les
//! [`ServerEvent`] et appelle [`play_event`] ; un autre remet les annonces à zéro à chaque nouvelle partie.

pub mod kills;
pub mod lab;
pub mod recipes;

use bevy::prelude::*;
use lov_shared::types::{AbilitySlot, DamageType, GameEvent};

use super::mixer::Channel;
use super::{AudioManager, SfxOpts};
use crate::net::{GameState, ServerEvent};
use kills::KillAnnouncer;
use recipes::{NoiseExt, ToneExt, at, noise, tone};

use super::synth::FilterKind::Bandpass;
use super::synth::Wave::{Sawtooth, Sine, Square, Triangle};

/// Enregistre la table événement → son (appelé par `SoundPlugin`).
pub fn plugin(app: &mut App) {
    app.add_message::<ServerEvent>()
        .init_resource::<KillAnnouncer>()
        .add_systems(
            Update,
            (reset_on_new_game, play_server_events)
                .chain()
                .run_if(resource_exists::<GameState>),
        );
}

/// `resetKillAnnouncements()` à chaque nouvelle partie.
fn reset_on_new_game(state: Res<GameState>, mut kills: ResMut<KillAnnouncer>) {
    kills.track_game(&state);
}

/// `audio.handleEvent(e)` pour chaque événement publié.
fn play_server_events(
    mut events: MessageReader<ServerEvent>,
    state: Res<GameState>,
    mut audio: ResMut<AudioManager>,
    mut kills: ResMut<KillAnnouncer>,
) {
    for ServerEvent(e) in events.read() {
        play_event(&mut audio, &mut kills, &state, e);
    }
}

/// Clé de sort `${championId}_${slot}` : vraie si `key` vaut `champion` + `_` + `slot`, sans allocation.
fn is_ability_key(key: &str, champion: &str, slot: &str) -> bool {
    key.strip_prefix(champion)
        .and_then(|rest| rest.strip_prefix('_'))
        == Some(slot)
}

/// `playEvent(audio, e)`.
pub fn play_event(
    a: &mut AudioManager,
    kills: &mut KillAnnouncer,
    state: &GameState,
    e: &GameEvent,
) {
    let self_id = state.game.as_ref().map(|g| g.entity_id);
    let pos_of = |id| AudioManager::entity_pos(state, Some(id));
    let ui = SfxOpts {
        channel: Channel::Ui,
        ..SfxOpts::default()
    };
    match e {
        GameEvent::AutoAttack { source, ranged, .. } => {
            a.sfx(at(pos_of(*source)), |r| {
                if *ranged {
                    r.noise(noise(0.08, 0.15).filter(3500.0).slide_filter(1200.0));
                } else {
                    r.noise(noise(0.07, 0.22).filter(900.0))
                        .tone(tone(Square, 140.0, 0.06, 0.12).slide(90.0));
                }
            });
        }

        GameEvent::Damage {
            target,
            damage_type,
            crit,
            ..
        } => {
            let vol = if *crit == Some(true) { 0.3 } else { 0.18 };
            a.sfx(at(pos_of(*target)), |r| match damage_type {
                DamageType::Magic => {
                    r.tone(tone(Sawtooth, 700.0, 0.12, vol).slide(250.0).filter(2000.0));
                }
                DamageType::True => {
                    r.tone(tone(Square, 300.0, 0.1, vol).slide(120.0))
                        .noise(noise(0.08, vol * 0.7).filter(2500.0));
                }
                DamageType::Physical => {
                    r.noise(noise(0.08, vol).filter(1100.0))
                        .tone(tone(Square, 180.0, 0.08, vol * 0.8).slide(100.0));
                }
            });
        }

        GameEvent::Heal { target, .. } => {
            a.sfx(at(pos_of(*target)), |r| {
                r.tone(tone(Sine, 620.0, 0.25, 0.15).slide(930.0));
            });
        }
        GameEvent::Shield { target, .. } => {
            a.sfx(at(pos_of(*target)), |r| {
                r.tone(tone(Triangle, 440.0, 0.2, 0.15).slide(660.0));
            });
        }

        GameEvent::Death { target, killer, .. } => {
            a.sfx(at(pos_of(*target)), |r| {
                r.noise(noise(0.5, 0.3).filter(800.0).slide_filter(120.0))
                    .tone(tone(Sawtooth, 160.0, 0.5, 0.25).slide(50.0).filter(600.0));
            });
            // les séries sont tenues même si la polyphonie a refusé le son de mort
            if let Some(sting) = kills.on_death(state, *target, *killer, a.now) {
                kills.play(a, sting);
            }
        }

        GameEvent::Respawn { target, .. } => {
            a.sfx(at(pos_of(*target)), |r| {
                r.tone(tone(Sine, 523.0, 0.3, 0.2))
                    .tone(tone(Sine, 784.0, 0.35, 0.18).delay(0.12));
            });
        }

        GameEvent::LevelUp { target, .. } => {
            let opts = SfxOpts {
                pos: pos_of(*target),
                vol: if Some(*target) == self_id { 1.0 } else { 0.7 },
                channel: Channel::Sfx,
            };
            a.sfx(opts, |r| {
                r.tone(tone(Sine, 523.0, 0.15, 0.22))
                    .tone(tone(Sine, 659.0, 0.15, 0.22).delay(0.1))
                    .tone(tone(Sine, 784.0, 0.3, 0.25).delay(0.2));
            });
        }

        GameEvent::GoldGain { target, .. } if Some(*target) == self_id => {
            a.sfx(ui, |r| {
                r.tone(tone(Square, 1320.0, 0.06, 0.12))
                    .tone(tone(Square, 1760.0, 0.09, 0.12).delay(0.05));
            });
        }

        GameEvent::ItemBought { target, .. } if Some(*target) == self_id => {
            a.sfx(ui, |r| {
                r.tone(tone(Square, 880.0, 0.07, 0.18))
                    .tone(tone(Square, 1320.0, 0.12, 0.18).delay(0.06));
            });
        }
        GameEvent::ItemSold { target, .. } if Some(*target) == self_id => {
            a.sfx(ui, |r| {
                r.tone(tone(Square, 660.0, 0.15, 0.15).slide(440.0));
            });
        }
        GameEvent::ItemUsed { target, item_id } => {
            a.sfx(at(pos_of(*target)), |r| {
                if item_id == "health_potion" {
                    r.noise(noise(0.3, 0.15).filter(900.0).kind(Bandpass));
                } else {
                    r.tone(tone(Triangle, 500.0, 0.2, 0.2).slide(750.0));
                }
            });
        }

        GameEvent::RecallStart { target } => {
            a.sfx(at(pos_of(*target)), |r| {
                r.tone(tone(Sine, 300.0, 1.2, 0.18).slide(900.0));
            });
        }
        GameEvent::RecallEnd { target, completed } => {
            a.sfx(at(pos_of(*target)), |r| {
                if *completed {
                    r.tone(tone(Sine, 880.0, 0.3, 0.22).slide(1320.0))
                        .tone(tone(Sine, 1760.0, 0.25, 0.15).delay(0.15));
                } else {
                    r.tone(tone(Square, 400.0, 0.2, 0.15).slide(200.0));
                }
            });
        }

        GameEvent::AbilityCast {
            source,
            champion_id,
            slot,
            target_pos,
            ..
        } => {
            let (upper, lower) = match slot {
                AbilitySlot::Q => ("Q", "q"),
                AbilitySlot::W => ("W", "w"),
                AbilitySlot::E => ("E", "e"),
                AbilitySlot::R => ("R", "r"),
            };
            // Même ordre que le web : la clé `${championId}_${slot}` garde le slot en majuscule, donc les
            // recettes dédiées (`grondak_r`…) ne sont jamais trouvées ici et la recette procédurale de
            // la clé en minuscule l'emporte. Porté tel quel (voir la PR).
            let recipe = recipes::custom(|k| is_ability_key(k, champion_id, upper))
                .or_else(|| recipes::procedural(|k| is_ability_key(k, champion_id, upper)))
                .or_else(|| recipes::procedural(|k| is_ability_key(k, champion_id, lower)));
            let pos = pos_of(*source).or(*target_pos);
            match recipe {
                Some(recipe) => recipe.play(a, pos, Some(1.0)),
                None => {
                    a.sfx(at(pos), |r| {
                        r.tone(tone(Square, 440.0, 0.15, 0.2).slide(660.0));
                    });
                }
            }
        }

        GameEvent::SummonerCast {
            source,
            spell,
            from,
            ..
        } => recipes::summoner(a, *spell, pos_of(*source).or(Some(*from))),

        GameEvent::ProjectileHit { pos, .. } => {
            a.sfx(at(Some(*pos)), |r| {
                r.noise(noise(0.1, 0.2).filter(2200.0).slide_filter(500.0))
                    .tone(tone(Sine, 500.0, 0.1, 0.12).slide(200.0));
            });
        }

        GameEvent::Dash { from, duration, .. } => {
            a.sfx(at(Some(*from)), |r| {
                r.noise(
                    noise(duration.min(0.4), 0.2)
                        .filter(2000.0)
                        .kind(Bandpass)
                        .slide_filter(500.0),
                );
            });
        }

        GameEvent::AreaEffect {
            fx_key,
            pos,
            duration,
            ..
        } => {
            // les fxKey avec recette dédiée gèrent déjà leur impact (ex. ult de Mélenchon)
            if !recipes::is_custom_fx_key(fx_key) {
                a.sfx(at(Some(*pos)), |r| {
                    r.tone(
                        tone(Sawtooth, 220.0, duration.min(0.6), 0.15)
                            .slide(440.0)
                            .filter(1500.0),
                    );
                });
            }
        }

        GameEvent::BuffApplied { target, .. } | GameEvent::BuffRemoved { target, .. } => {
            let applied = matches!(e, GameEvent::BuffApplied { .. });
            let (freq, slide) = if applied { (700.0, 1.3) } else { (350.0, 0.7) };
            a.sfx(at(pos_of(*target)), |r| {
                r.tone(tone(Sine, freq, 0.15, 0.1).slide(slide));
            });
        }

        GameEvent::StructureDestroyed { target, .. } => {
            a.sfx(at(pos_of(*target)), |r| {
                r.noise(noise(1.2, 0.4).filter(600.0).slide_filter(80.0))
                    .tone(tone(Sawtooth, 90.0, 1.0, 0.3).slide(30.0).filter(400.0));
            });
            // Le web cherche une annonce vocale `tower_destroyed`, absente de sa table : rien à dire.
        }

        GameEvent::ObjectiveTaken { .. } => {
            a.sfx(SfxOpts::default(), |r| {
                r.tone(tone(Sawtooth, 392.0, 0.2, 0.2).filter(2000.0))
                    .tone(tone(Sine, 523.0, 0.2, 0.2).delay(0.15).filter(2000.0))
                    .tone(tone(Sine, 659.0, 0.35, 0.22).delay(0.3).filter(2000.0));
            });
        }

        GameEvent::Announcement { .. } => {
            // Carillon sur le canal Voix. Le web lit ensuite le texte par `speechSynthesis`, que le client
            // natif n'a pas : le texte reste affiché par l'interface.
            let voice = SfxOpts {
                channel: Channel::Voice,
                ..SfxOpts::default()
            };
            a.sfx(voice, |r| {
                r.tone(tone(Triangle, 660.0, 0.12, 0.2))
                    .tone(tone(Triangle, 880.0, 0.2, 0.2).delay(0.12));
            });
        }

        GameEvent::Ping { pos, .. } => {
            let opts = SfxOpts {
                pos: Some(*pos),
                ..ui
            };
            a.sfx(opts, |r| {
                r.tone(tone(Sine, 1200.0, 0.1, 0.25))
                    .tone(tone(Sine, 1200.0, 0.15, 0.2).delay(0.12));
            });
        }

        _ => {}
    }
}

#[cfg(test)]
mod tests;
