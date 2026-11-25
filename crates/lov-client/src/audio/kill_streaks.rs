//! Suivi des éliminations **entre champions** (pur, testé) : multi-kills (fenêtre de 10 s) et séries
//! (kills sans mourir). Les morts de sbires, monstres ou structures ne passent jamais ici.
//!
//! Portage de `packages/client/src/audio/killStreaks.ts`.

use std::collections::HashMap;

use lov_shared::types::EntityId;

/// Secondes entre deux kills d'un même multi-kill.
pub const MULTI_WINDOW: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KillInfo {
    /// 1 = kill simple, 2 = double… (plafonné à 5).
    pub multi: u32,
    /// Kills consécutifs du tueur sans mourir.
    pub spree: u32,
    pub first_blood: bool,
}

#[derive(Debug, Default)]
pub struct KillStreaks {
    /// Par tueur : nombre de kills du multi-kill en cours et instant du dernier.
    multi: HashMap<EntityId, (u32, f64)>,
    spree: HashMap<EntityId, u32>,
    first_blood_done: bool,
}

impl KillStreaks {
    pub fn reset(&mut self) {
        self.multi.clear();
        self.spree.clear();
        self.first_blood_done = false;
    }

    /// Un champion (`victim`) a été tué par un champion ennemi (`killer`) à l'instant `now` (secondes).
    pub fn on_champion_kill(&mut self, victim: EntityId, killer: EntityId, now: f64) -> KillInfo {
        self.spree.remove(&victim);
        self.multi.remove(&victim);
        let count = match self.multi.get(&killer) {
            Some(&(count, last)) if now - last <= MULTI_WINDOW => count + 1,
            _ => 1,
        };
        self.multi.insert(killer, (count, now));
        let spree = self.spree.get(&killer).copied().unwrap_or(0) + 1;
        self.spree.insert(killer, spree);
        let first_blood = !self.first_blood_done;
        self.first_blood_done = true;
        KillInfo {
            multi: count.min(5),
            spree,
            first_blood,
        }
    }

    /// Mort sans tueur champion (tour, sbire, monstre) : la série de la victime s'arrête aussi.
    pub fn on_champion_death(&mut self, victim: EntityId) {
        self.spree.remove(&victim);
        self.multi.remove(&victim);
    }
}

/// Niveau d'escalade du jingle (0..9) : le multi-kill domine, la série ajoute un cran tous les 2 kills.
/// Kill simple sans série = 0 ; penta = 8 et plus.
pub fn sting_level(info: KillInfo) -> u32 {
    let from_multi = (info.multi.max(1) - 1) * 2;
    let from_spree = (info.spree.saturating_sub(1) / 2).min(3);
    (from_multi + from_spree).min(9)
}

/// Portage de `packages/client/test/kill-audio.test.ts` (partie séries ; les jingles viennent avec la musique).
#[cfg(test)]
mod tests {
    use super::*;

    fn info(multi: u32, spree: u32, first_blood: bool) -> KillInfo {
        KillInfo {
            multi,
            spree,
            first_blood,
        }
    }

    #[test]
    fn premier_sang_puis_multi_kill_dans_la_fenetre_de_10_s() {
        let mut k = KillStreaks::default();
        assert_eq!(k.on_champion_kill(10, 1, 0.0), info(1, 1, true));
        assert_eq!(k.on_champion_kill(11, 1, 4.0), info(2, 2, false));
        let third = k.on_champion_kill(12, 1, 9.0);
        assert_eq!((third.multi, third.spree), (3, 3));
        // hors fenêtre : la série continue
        let fourth = k.on_champion_kill(13, 1, 30.0);
        assert_eq!((fourth.multi, fourth.spree), (1, 4));
    }

    #[test]
    fn la_mort_du_tueur_remet_sa_serie_a_zero_meme_executee_par_une_tour() {
        let mut k = KillStreaks::default();
        k.on_champion_kill(10, 1, 0.0);
        k.on_champion_kill(11, 1, 20.0);
        k.on_champion_death(1);
        let after = k.on_champion_kill(12, 1, 60.0);
        assert_eq!((after.multi, after.spree), (1, 1));
    }

    #[test]
    fn le_jingle_monte_en_puissance_avec_le_multi_kill_et_la_serie() {
        let lvl = |multi, spree| sting_level(info(multi, spree, false));
        assert_eq!(lvl(1, 1), 0);
        assert_eq!(lvl(1, 3), 1);
        assert_eq!(lvl(2, 2), 2);
        assert_eq!(lvl(3, 3), 5);
        assert_eq!(lvl(5, 5), 9);
        // strictement croissant sur un massacre
        let seq: Vec<u32> = (1..=5).map(|n| lvl(n, n)).collect();
        assert!(seq.windows(2).all(|w| w[1] > w[0]), "{seq:?}");
    }
}
