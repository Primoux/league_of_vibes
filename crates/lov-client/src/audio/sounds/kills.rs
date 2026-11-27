//! Annonces d'élimination : uniquement un champion tué par un champion ennemi (jamais les sbires,
//! monstres ou tours). Pas de voix : un jingle synthétisé qui gagne en ampleur avec le multi-kill et la
//! série ([`KillStreaks`]), joué sur un bus de musique du canal Voix au gain global 1.
//!
//! Portage de `onDeath`, `resetKillAnnouncements` et `getStingBus` de `sounds.ts`.

use std::collections::HashMap;

use bevy::prelude::Resource;
use lov_shared::types::{EntityId, EntityKind, Team, TeamOrNeutral};

use super::super::AudioManager;
use super::super::kill_streaks::{KillStreaks, sting_level};
use super::super::mixer::Channel;
use super::super::music::stings::{StingSide, first_blood_sting, kill_sting};
use super::super::music::{BusCommand, MusicBusHandle, Score};
use crate::net::GameState;

/// Le jingle à jouer pour un kill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sting {
    FirstBlood(StingSide),
    /// Niveau 0..9 ([`sting_level`]).
    Kill(u32, StingSide),
}

/// État des annonces d'une partie.
#[derive(Resource, Debug, Default)]
pub struct KillAnnouncer {
    streaks: KillStreaks,
    /// Équipe de chaque champion vu (les ids restent connus même quand l'unité passe dans le brouillard).
    champ_teams: HashMap<EntityId, Team>,
    /// Bus des jingles, créé au premier kill et gardé d'une partie à l'autre (le mixeur en héberge 4).
    bus: Option<MusicBusHandle>,
    /// `serverTime` du dernier `GameStart` vu (bits du `f64`), `None` hors partie (`wasInGame`).
    game: Option<u64>,
    /// Un résultat a été vu depuis ce `GameStart` (`hadResult`).
    had_result: bool,
}

impl KillAnnouncer {
    /// `resetKillAnnouncements()`.
    pub fn reset(&mut self) {
        self.streaks.reset();
        self.champ_teams.clear();
    }

    /// Remet les annonces à zéro à chaque nouvelle partie (le `store.subscribe` d'`installAudio`).
    ///
    /// Le web réagit au passage de `game` de vide à rempli. Ici, `game` n'est jamais vidé : un nouveau
    /// `GameStart` le remplace. Un `GameStart` au `serverTime` différent compte donc comme une nouvelle
    /// partie s'il suit un résultat ; sans résultat, c'est le même `GameStart` renvoyé à un joueur qui
    /// revient (`resync` du serveur), et le web ne remet rien à zéro non plus. Renvoie `true` à la remise
    /// à zéro.
    pub fn track_game(&mut self, state: &GameState) -> bool {
        self.had_result |= state.result.is_some();
        match &state.game {
            Some(g) => {
                let start = g.server_time.to_bits();
                let new_game = match self.game {
                    None => true,
                    Some(last) => last != start && self.had_result,
                };
                self.game = Some(start);
                if new_game {
                    self.had_result = state.result.is_some();
                    self.reset();
                }
                new_game
            }
            None => {
                if state.result.is_none() {
                    self.game = None;
                    self.had_result = false;
                }
                false
            }
        }
    }

    fn refresh_champions(&mut self, state: &GameState) {
        for e in state.entities().values() {
            if e.kind != EntityKind::Champion {
                continue;
            }
            let team = match e.team {
                TeamOrNeutral::Blue => Team::Blue,
                TeamOrNeutral::Red => Team::Red,
                TeamOrNeutral::Neutral => continue,
            };
            self.champ_teams.insert(e.id, team);
        }
    }

    /// `onDeath` sans le son : met à jour les séries et décide du jingle (`now` en secondes).
    pub fn on_death(
        &mut self,
        state: &GameState,
        victim: EntityId,
        killer: Option<EntityId>,
        now: f64,
    ) -> Option<Sting> {
        self.refresh_champions(state);
        // pas un champion : aucune annonce
        let victim_team = *self.champ_teams.get(&victim)?;
        let killer_team = killer.and_then(|k| self.champ_teams.get(&k).copied());
        let (Some(killer), Some(killer_team)) = (killer, killer_team) else {
            // exécuté par une tour / un sbire / un monstre
            self.streaks.on_champion_death(victim);
            return None;
        };
        if killer_team == victim_team {
            self.streaks.on_champion_death(victim);
            return None;
        }
        let info = self.streaks.on_champion_kill(victim, killer, now);
        let me = state
            .self_state
            .as_ref()
            .map(|s| s.entity_id)
            .or(state.game.as_ref().map(|g| g.entity_id));
        let my_team = state.game.as_ref().map(|g| g.team);
        let side = if Some(killer) == me {
            StingSide::Own
        } else if Some(killer_team) == my_team {
            StingSide::Ally
        } else {
            StingSide::Enemy
        };
        Some(if info.first_blood {
            Sting::FirstBlood(side)
        } else {
            Sting::Kill(sting_level(info), side)
        })
    }

    /// Joue le jingle sur le bus du canal Voix, créé au besoin (`getStingBus`).
    pub fn play(&mut self, a: &mut AudioManager, sting: Sting) {
        let bus = match &self.bus {
            Some(bus) => bus.clone(),
            None => {
                let bus = a.music_bus(Channel::Voice);
                a.bus(&bus, BusCommand::SetMaster(1.0));
                self.bus = Some(bus.clone());
                bus
            }
        };
        let t = bus.current_time() + 0.05;
        let mut score = Score::new();
        match sting {
            Sting::FirstBlood(side) => first_blood_sting(&mut score, t, side),
            Sting::Kill(level, side) => kill_sting(&mut score, t, level, side),
        }
        a.bus(&bus, BusCommand::Play(score));
    }
}
