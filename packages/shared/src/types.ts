/**
 * Types cœur partagés serveur/client. Propriétaire : claude-server.
 * Toute modification = commit `contract:` + note dans docs/progress.
 */

export type Team = 'blue' | 'red';
export const TEAMS: readonly Team[] = ['blue', 'red'];
export const otherTeam = (t: Team): Team => (t === 'blue' ? 'red' : 'blue');

export interface Vec2 {
  x: number;
  y: number;
}

export type EntityId = number;
export type PlayerId = string;
export type Lane = 'top' | 'mid' | 'bot';
export type Role = 'top' | 'jungle' | 'mid' | 'bot';

export type EntityKind =
  | 'champion'
  | 'minion'
  | 'tower'
  | 'inhibitor'
  | 'nexus'
  | 'monster'
  | 'ward'
  | 'projectile'
  | 'zone'; // zones persistantes (blizzard, piège...)

export type MinionType = 'melee' | 'caster' | 'cannon' | 'super';
export type TowerTier = 'outer' | 'inner' | 'inhib' | 'nexus';
export type MonsterType =
  | 'blue_buff'
  | 'red_buff'
  | 'wolves'
  | 'wolf_small'
  | 'raptors'
  | 'raptor_small'
  | 'krugs'
  | 'krug_small'
  | 'gromp'
  | 'scuttle'
  | 'dragon'
  | 'elder_dragon'
  | 'grub'
  | 'herald'
  | 'leviathan';

export type DragonElement = 'infernal' | 'ocean' | 'mountain' | 'cloud' | 'hextech' | 'chemtech';

export type AbilitySlot = 'Q' | 'W' | 'E' | 'R';
export type SummonerSlot = 'D' | 'F';
export type SummonerId =
  | 'flash'
  | 'ignite'
  | 'heal'
  | 'barrier'
  | 'exhaust'
  | 'teleport'
  | 'smite'
  | 'ghost'
  | 'cleanse';

export type DamageType = 'physical' | 'magic' | 'true';

/** Statistiques calculées (base + niveau + objets + buffs). */
export interface Stats {
  maxHp: number;
  hpRegen: number; // par 5 s
  maxMana: number;
  manaRegen: number; // par 5 s
  ad: number;
  ap: number;
  armor: number;
  mr: number;
  attackSpeed: number; // attaques / s
  attackRange: number;
  moveSpeed: number;
  critChance: number; // 0..1
  abilityHaste: number;
  lifesteal: number; // 0..1
  armorPen: number; // flat (lethality simplifiée)
  magicPen: number; // flat
}

/** Ce que le client reçoit pour chaque entité visible. */
export interface EntitySnapshot {
  id: EntityId;
  kind: EntityKind;
  team: Team | 'neutral';
  x: number;
  y: number;
  facing: number; // radians, 0 = +x
  hp: number;
  maxHp: number;
  mana?: number;
  maxMana?: number;
  shield?: number;
  dead?: boolean;
  /** Type précis : championId, MinionType, TowerTier, MonsterType, etc. */
  subtype?: string;
  ownerId?: PlayerId;
  level?: number;
  /** Animation courante pour le client (idle/run/attack/cast_Q/death...). */
  anim?: string;
  buffs?: { id: string; remaining: number; stacks?: number }[];
  /** Données champion visibles par tous. */
  champ?: {
    playerId: PlayerId;
    name: string;
    kills: number;
    deaths: number;
    assists: number;
    cs: number;
    items: (string | null)[];
    respawnIn?: number;
  };
}

/** Données privées envoyées uniquement au joueur propriétaire. */
export interface SelfState {
  entityId: EntityId;
  gold: number;
  xp: number;
  xpToNext: number;
  level: number;
  skillPoints: number;
  abilityLevels: Record<AbilitySlot, number>;
  cooldowns: Record<AbilitySlot | SummonerSlot, number>; // secondes restantes
  summoners: Record<SummonerSlot, SummonerId>;
  stats: Stats;
  recalling?: number; // secondes restantes
  canShop: boolean;
}

/**
 * Événements émis par la simulation (EventBus serveur), transmis aux clients dans les snapshots.
 * C'est la base des FX (claude-fx), de l'audio (mistral-economy) et des annonces UI (mistral-ui).
 */
export type GameEvent =
  | { type: 'damage'; source: EntityId; target: EntityId; amount: number; damageType: DamageType; crit?: boolean; ability?: string }
  | { type: 'heal'; source: EntityId; target: EntityId; amount: number }
  | { type: 'shield'; source: EntityId; target: EntityId; amount: number }
  | { type: 'death'; target: EntityId; killer?: EntityId; assists?: EntityId[]; bounty?: number }
  | { type: 'respawn'; target: EntityId; pos: Vec2 }
  | { type: 'autoAttack'; source: EntityId; target: EntityId; ranged: boolean; /** vitesse du projectile (distance) */ speed?: number; crit?: boolean }
  | { type: 'abilityCast'; source: EntityId; championId: string; slot: AbilitySlot; abilityId: string; targetPos?: Vec2; targetId?: EntityId }
  | { type: 'summonerCast'; source: EntityId; spell: SummonerId; from: Vec2; to?: Vec2; targetId?: EntityId }
  | { type: 'projectileSpawn'; id: EntityId; source: EntityId; fxKey: string; from: Vec2; to: Vec2; speed: number; width: number }
  | { type: 'projectileHit'; id: EntityId; target?: EntityId; pos: Vec2; fxKey: string }
  | { type: 'dash'; source: EntityId; from: Vec2; to: Vec2; duration: number; fxKey?: string }
  | { type: 'areaEffect'; source: EntityId; fxKey: string; pos: Vec2; radius: number; duration: number }
  | { type: 'buffApplied'; target: EntityId; buffId: string; duration: number; source?: EntityId }
  | { type: 'buffRemoved'; target: EntityId; buffId: string }
  | { type: 'levelUp'; target: EntityId; level: number }
  | { type: 'goldGain'; target: EntityId; amount: number; pos?: Vec2 }
  | { type: 'recallStart'; target: EntityId }
  | { type: 'recallEnd'; target: EntityId; completed: boolean }
  | { type: 'itemBought'; target: EntityId; itemId: string }
  | { type: 'itemSold'; target: EntityId; itemId: string }
  | { type: 'structureDestroyed'; target: EntityId; kind: 'tower' | 'inhibitor' | 'nexus'; team: Team; plate?: boolean }
  | { type: 'objectiveSpawn'; monster: MonsterType; element?: DragonElement; pos: Vec2 }
  | { type: 'objectiveTaken'; monster: MonsterType; team: Team; killer?: EntityId; element?: DragonElement }
  | { type: 'announcement'; key: string; team?: Team; params?: Record<string, string | number> }
  | { type: 'ping'; playerId: PlayerId; pos: Vec2; kind: string }
  | { type: 'chat'; playerId: PlayerId; text: string; teamOnly: boolean };

export type GameEventType = GameEvent['type'];
