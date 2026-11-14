//! Constantes globales du jeu (port de `constants.ts`). Unités « LoL-like ».

/// Ticks de simulation par seconde (serveur).
pub const TICK_RATE: u32 = 30;
pub const TICK_DT: f64 = 1.0 / TICK_RATE as f64;
/// Snapshots par seconde envoyés aux clients. DOIT diviser `TICK_RATE`.
pub const SNAPSHOT_RATE: u32 = 30;
/// Délai d'interpolation côté client.
pub const INTERP_DELAY_MS: u32 = 100;

/// Carte carrée, origine (0,0) = coin bas-gauche (base bleue).
pub const MAP_SIZE: f64 = 12000.0;
/// 1 unité de jeu = 0.01 unité Three.js.
pub const RENDER_SCALE: f64 = 1.0 / 100.0;

pub const TEAM_SIZE: u32 = 4;
pub const MAX_PLAYERS: u32 = TEAM_SIZE * 2;
pub const MAX_LEVEL: u32 = 18;
pub const ULT_LEVELS: [u32; 3] = [6, 11, 16];
pub const INVENTORY_SLOTS: u32 = 6;

pub const STARTING_GOLD: f64 = 500.0;
pub const PASSIVE_GOLD_PER_10S: f64 = 20.4;
/// Secondes (non scalé).
pub const PASSIVE_GOLD_START: f64 = 65.0;

pub const DRAFT_TURN_SECONDS: f64 = 30.0;
pub const LOADING_SECONDS: f64 = 5.0;
pub const RECALL_SECONDS: f64 = 8.0;
pub const FOUNTAIN_RADIUS: f64 = 900.0;

/// Compression du temps : tous les timers d'objectifs/vagues/respawn de structures sont multipliés
/// par `TIME_SCALE`.
pub const TIME_SCALE: f64 = 0.75;

/// Timers en secondes de jeu, NON scalés (valeurs LoL de référence). Utiliser [`scaled`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timers {
    pub minion_first_wave: f64,
    pub minion_wave_interval: f64,
    /// 1 vague sur 3 au début.
    pub cannon_every_n_waves: u32,
    /// Après `cannon_late_at`.
    pub cannon_every_n_waves_late: u32,
    pub cannon_late_at: f64,
    pub tower_plates_until: f64,
    pub inhibitor_respawn: f64,
    pub jungle_first_spawn: f64,
    pub jungle_buff_respawn: f64,
    pub jungle_small_respawn: f64,
    pub scuttle_first_spawn: f64,
    pub scuttle_respawn: f64,
    pub dragon_first_spawn: f64,
    pub dragon_respawn: f64,
    pub elder_respawn: f64,
    pub grubs_spawn: f64,
    pub herald_spawn: f64,
    pub herald_despawn: f64,
    pub leviathan_spawn: f64,
    pub leviathan_respawn: f64,
    pub leviathan_buff_duration: f64,
    pub elder_buff_duration: f64,
}

pub const TIMERS: Timers = Timers {
    minion_first_wave: 65.0,
    minion_wave_interval: 30.0,
    cannon_every_n_waves: 3,
    cannon_every_n_waves_late: 2,
    cannon_late_at: 15.0 * 60.0,
    tower_plates_until: 14.0 * 60.0,
    inhibitor_respawn: 5.0 * 60.0,
    jungle_first_spawn: 90.0,
    jungle_buff_respawn: 5.0 * 60.0,
    jungle_small_respawn: 135.0,
    scuttle_first_spawn: 3.0 * 60.0 + 30.0,
    scuttle_respawn: 150.0,
    dragon_first_spawn: 5.0 * 60.0,
    dragon_respawn: 5.0 * 60.0,
    elder_respawn: 6.0 * 60.0,
    grubs_spawn: 8.0 * 60.0,
    herald_spawn: 15.0 * 60.0,
    herald_despawn: 19.0 * 60.0 + 45.0,
    leviathan_spawn: 20.0 * 60.0,
    leviathan_respawn: 6.0 * 60.0,
    leviathan_buff_duration: 180.0,
    elder_buff_duration: 150.0,
};

pub const fn scaled(seconds: f64) -> f64 {
    seconds * TIME_SCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constantes_derivees() {
        assert_eq!(MAX_PLAYERS, 8);
        assert_eq!(TICK_RATE % SNAPSHOT_RATE, 0);
        assert!((TICK_DT - 1.0 / 30.0).abs() < f64::EPSILON);
        assert_eq!(ULT_LEVELS, [6, 11, 16]);
    }

    #[test]
    fn timers_scales() {
        assert_eq!(scaled(TIMERS.dragon_first_spawn), 225.0);
        assert_eq!(scaled(TIMERS.herald_despawn), 888.75);
        assert_eq!(TIMERS.scuttle_first_spawn, 210.0);
        assert_eq!(TIMERS.cannon_late_at, 900.0);
    }
}
