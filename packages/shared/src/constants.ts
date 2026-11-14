/**
 * Constantes globales du jeu. Les unités sont « LoL-like » (portée AA 550, vitesse 335, etc.).
 * Propriétaire des timers : codex-objectives. Propriétaire du reste : claude-server.
 */

export const TICK_RATE = 30; // ticks de simulation / seconde (serveur)
export const TICK_DT = 1 / TICK_RATE;
/**
 * Snapshots / seconde envoyés aux clients. DOIT diviser TICK_RATE (cadence régulière : un snapshot tous les
 * TICK_RATE / SNAPSHOT_RATE ticks). 30 = un snapshot par tick : mesuré avec netcheck (gigue ±30 ms), la marge du
 * tampon d'interpolation passe de −5 ms (15 Hz) à 24 ms ; le coût en débit est absorbé par les patches delta.
 */
export const SNAPSHOT_RATE = 30;
export const INTERP_DELAY_MS = 100; // délai d'interpolation côté client

export const MAP_SIZE = 12000; // carte carrée, origine (0,0) = coin bas-gauche (base bleue)
export const RENDER_SCALE = 1 / 100; // 1 unité de jeu = 0.01 unité Three.js

export const TEAM_SIZE = 4;
export const MAX_PLAYERS = TEAM_SIZE * 2;
export const MAX_LEVEL = 18;
export const ULT_LEVELS = [6, 11, 16] as const;
export const INVENTORY_SLOTS = 6;

export const STARTING_GOLD = 500;
export const PASSIVE_GOLD_PER_10S = 20.4;
export const PASSIVE_GOLD_START = 65; // secondes (non scalé)

export const DRAFT_TURN_SECONDS = 30;
export const LOADING_SECONDS = 5;
export const RECALL_SECONDS = 8;
export const FOUNTAIN_RADIUS = 900;

/**
 * Compression du temps : tous les timers d'objectifs/vagues/respawn de structures sont multipliés par TIME_SCALE.
 * 0.75 => parties d'environ 25-30 minutes.
 */
export const TIME_SCALE = 0.75;

/** Timers en secondes de jeu, NON scalés (valeurs LoL de référence). Utiliser scaled(). */
export const TIMERS = {
  minionFirstWave: 65,
  minionWaveInterval: 30,
  cannonEveryNWaves: 3, // 1 vague sur 3 au début
  cannonEveryNWavesLate: 2, // après 15:00
  cannonLateAt: 15 * 60,
  towerPlatesUntil: 14 * 60,
  inhibitorRespawn: 5 * 60,
  jungleFirstSpawn: 90,
  jungleBuffRespawn: 5 * 60,
  jungleSmallRespawn: 135,
  scuttleFirstSpawn: 3 * 60 + 30,
  scuttleRespawn: 150,
  dragonFirstSpawn: 5 * 60,
  dragonRespawn: 5 * 60,
  elderRespawn: 6 * 60,
  grubsSpawn: 8 * 60,
  heraldSpawn: 15 * 60,
  heraldDespawn: 19 * 60 + 45,
  leviathanSpawn: 20 * 60,
  leviathanRespawn: 6 * 60,
  leviathanBuffDuration: 180,
  elderBuffDuration: 150,
} as const;

export const scaled = (seconds: number): number => seconds * TIME_SCALE;
