//! Bus FX côté client : portage de `packages/client/src/fx/FxBus.ts`.
//!
//! Le réseau publie déjà chaque événement en [`ServerEvent`](crate::net::ServerEvent) au moment où le rendu
//! interpolé atteint son snapshot : `delay_ms` vaut donc 0 par défaut. Si une source publiait des événements
//! « bruts » (dès réception), `delay_ms = INTERP_DELAY_MS` les retarderait pour la livraison normale ; la
//! livraison immédiate les reçoit toujours sans délai.
//!
//! Écart de forme : le web appelle des abonnés ; ici le bus range les événements dans deux sorties que le
//! consommateur vide à chaque image ([`FxBus::take_immediate`], [`FxBus::take_ready`]). Seul le dispatcher
//! FX consomme le bus, et Bevy distribue déjà `ServerEvent` à qui veut le lire.

use std::collections::VecDeque;

use bevy::prelude::*;
use lov_shared::types::GameEvent;

/// File des événements FX (`FxBus`).
#[derive(Resource, Debug, Default)]
pub struct FxBus {
    /// Délai appliqué à la livraison normale (ms). 0 = livraison directe.
    pub delay_ms: f64,
    /// Remplir la livraison immédiate. Désactivé par défaut : sans lecteur, chaque événement y serait
    /// copié pour rien.
    pub immediate_enabled: bool,
    queue: VecDeque<(f64, GameEvent)>,
    immediate: Vec<GameEvent>,
    ready: Vec<GameEvent>,
}

impl FxBus {
    /// Reçoit un événement à l'instant `now` (ms).
    pub fn dispatch(&mut self, e: GameEvent, now: f64) {
        if self.immediate_enabled {
            self.immediate.push(e.clone());
        }
        if self.delay_ms <= 0.0 {
            self.ready.push(e);
        } else {
            self.queue.push_back((now + self.delay_ms, e));
        }
    }

    /// Livre les événements dont le délai est écoulé (ordre de réception conservé).
    pub fn update(&mut self, now: f64) {
        while self.queue.front().is_some_and(|(at, _)| *at <= now) {
            if let Some((_, e)) = self.queue.pop_front() {
                self.ready.push(e);
            }
        }
    }

    /// Événements en attente de leur délai.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Événements reçus depuis le dernier appel, sans délai (`subscribe(fn, { immediate: true })`).
    pub fn take_immediate(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.immediate)
    }

    /// Événements livrés (délai écoulé) depuis le dernier appel.
    pub fn take_ready(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.ready)
    }

    /// Rend au bus le tampon vidé par [`FxBus::take_ready`], pour réutiliser son allocation.
    pub fn recycle(&mut self, mut buf: Vec<GameEvent>) {
        if self.ready.is_empty() {
            buf.clear();
            self.ready = buf;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lvl(level: u32) -> GameEvent {
        GameEvent::LevelUp {
            target: 1,
            level,
            fx_key: None,
        }
    }

    fn levels(v: &[GameEvent]) -> Vec<u32> {
        v.iter()
            .filter_map(|e| match e {
                GameEvent::LevelUp { level, .. } => Some(*level),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn retarde_la_livraison_normale_pas_l_immediate() {
        let mut bus = FxBus {
            delay_ms: 100.0,
            immediate_enabled: true,
            ..default()
        };
        let mut late = Vec::new();
        bus.dispatch(lvl(2), 1000.0);
        bus.dispatch(lvl(3), 1050.0);
        assert_eq!(levels(&bus.take_immediate()), [2, 3]);
        bus.update(1099.0);
        late.extend(bus.take_ready());
        assert!(late.is_empty());
        bus.update(1100.0);
        late.extend(bus.take_ready());
        assert_eq!(levels(&late), [2]);
        bus.update(2000.0);
        late.extend(bus.take_ready());
        assert_eq!(levels(&late), [2, 3]);
        assert_eq!(bus.pending(), 0);
    }

    #[test]
    fn livre_directement_sans_delai() {
        let mut bus = FxBus::default();
        bus.dispatch(lvl(5), 0.0);
        assert_eq!(bus.take_ready().len(), 1);
    }

    #[test]
    fn une_sortie_videe_ne_relivre_pas() {
        let mut bus = FxBus {
            immediate_enabled: true,
            ..default()
        };
        bus.dispatch(lvl(2), 0.0);
        let first = bus.take_ready();
        bus.recycle(first);
        assert!(bus.take_ready().is_empty());
        assert!(bus.take_immediate().len() == 1 && bus.take_immediate().is_empty());
        // sans lecteur de la livraison immédiate, rien n'y est copié
        let mut quiet = FxBus::default();
        quiet.dispatch(lvl(3), 0.0);
        assert!(quiet.take_immediate().is_empty());
    }
}
