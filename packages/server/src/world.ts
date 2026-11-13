/**
 * Monde de simulation. Propriétaire : claude-server.
 * Les autres agents ajoutent leur logique dans systems/<leur-fichier>.ts et lisent/écrivent les entités ici.
 * Champs spécifiques à un domaine : utiliser `entity.data` (namespacé, ex. data.minion, data.tower) plutôt que
 * d'ajouter des champs au type Entity sans commit `contract:`.
 */
import {
  TICK_DT,
  getChampion,
  type AbilitySlot,
  type ClientMsg,
  type EntityId,
  type EntityKind,
  type GameEvent,
  type PlayerId,
  type Stats,
  type SummonerId,
  type SummonerSlot,
  type Team,
  type Vec2,
} from '@lov/shared';

export interface ChampionState {
  playerId: PlayerId;
  championId: string;
  name: string;
  level: number;
  xp: number;
  skillPoints: number;
  abilityLevels: Record<AbilitySlot, number>;
  cooldowns: Record<AbilitySlot | SummonerSlot, number>;
  summoners: Record<SummonerSlot, SummonerId>;
  gold: number;
  items: (string | null)[];
  kills: number;
  deaths: number;
  assists: number;
  cs: number;
  respawnIn: number;
  damageToChampions: number;
}

export interface Entity {
  id: EntityId;
  kind: EntityKind;
  team: Team | 'neutral';
  pos: Vec2;
  facing: number;
  radius: number;
  hp: number;
  mana: number;
  shield: number;
  /** Stats calculées pour ce tick (recalculées par buffs/leveling/economy). */
  stats: Stats;
  dead: boolean;
  subtype?: string;
  anim?: string;
  /** Ordre de déplacement courant (consommé par movement). */
  moveTarget?: Vec2;
  /** Chemin calculé par le pathfinding (claude-server). */
  path?: Vec2[];
  /** Cible d'attaque courante (consommée par combat). */
  attackTarget?: EntityId;
  champ?: ChampionState;
  /** Données propres à chaque domaine, namespacées par système (ex. data.minion, data.buffs). */
  data: Record<string, unknown>;
}

export interface QueuedCommand {
  playerId: PlayerId;
  msg: ClientMsg;
}

type Listener<T extends GameEvent['type']> = (e: Extract<GameEvent, { type: T }>) => void;

/** Bus d'événements de simulation. Chaque événement émis est aussi envoyé aux clients. */
export class EventBus {
  private listeners = new Map<string, Set<(e: GameEvent) => void>>();
  /** Événements du tick courant, vidés après chaque snapshot. */
  pending: GameEvent[] = [];

  on<T extends GameEvent['type']>(type: T, fn: Listener<T>): () => void {
    let set = this.listeners.get(type);
    if (!set) this.listeners.set(type, (set = new Set()));
    set.add(fn as (e: GameEvent) => void);
    return () => set.delete(fn as (e: GameEvent) => void);
  }

  emit(e: GameEvent): void {
    this.pending.push(e);
    this.listeners.get(e.type)?.forEach((fn) => fn(e));
  }

  drain(): GameEvent[] {
    const out = this.pending;
    this.pending = [];
    return out;
  }
}

/** Contexte mutable passé aux hooks de dégâts (avant mitigation). Voir combat/damage.ts. */
export interface DamageContext {
  source: Entity | undefined;
  /** Champion (ou entité) crédité : le propriétaire de la source si `source.data.owner` est défini. */
  credit: Entity | undefined;
  target: Entity;
  /** Montant brut avant résistances ; un hook peut le modifier (0 = annulé). */
  amount: number;
  type: import('@lov/shared').DamageType;
  ability?: string;
  isAutoAttack: boolean;
  crit: boolean;
  /** Pénétration additionnelle (flat) et en pourcentage (0..1) pour ce coup. */
  flatPen: number;
  percentPen: number;
  /** Multiplicateur final appliqué après résistances (ex. réduction de dégâts 0.7). */
  multiplier: number;
}

export interface HealContext {
  source: Entity | undefined;
  target: Entity;
  amount: number;
}

export interface AttackContext {
  attacker: Entity;
  target: Entity;
  /** Dégâts physiques de base (AD) ; modifiable. Le crit multiplie ce montant. */
  amount: number;
  crit: boolean;
  critMultiplier: number;
  /** Dégâts bonus ajoutés après le crit (on-hit), dans le même événement `damage`. */
  bonus: number;
  damageType: import('@lov/shared').DamageType;
}

/**
 * Points d'extension de la simulation : les systèmes des autres agents s'y enregistrent dans leur `init(world)`
 * au lieu d'éditer les systèmes de claude-server. Tous les tableaux sont exécutés dans l'ordre d'enregistrement.
 */
export interface WorldHooks {
  /** Avant mitigation : modifier ctx.amount / multiplier / pen (Exhaust, fortification, passif de Nyx, stase...). */
  beforeDamage: ((ctx: DamageContext, world: World) => void)[];
  /** Après application (dealt = PV + bouclier retirés). */
  afterDamage: ((ctx: DamageContext, dealt: number, world: World) => void)[];
  /** Modifie un soin (anti-soin, amplification). */
  beforeHeal: ((ctx: HealContext, world: World) => void)[];
  /** Retourne true pour empêcher une mort (l'entité doit alors avoir hp > 0, ex. stase / ange gardien). */
  preventDeath: ((target: Entity, killer: Entity | undefined, world: World) => boolean)[];
  /** Prime d'or d'un kill (mistral-economy). Le premier hook qui retourne un nombre gagne. */
  bounty: ((victim: Entity, killer: Entity | undefined, assists: Entity[], world: World) => number | undefined)[];
  /** Durée de mort d'un champion (s). Le premier hook qui retourne un nombre gagne ; sinon formule par défaut. */
  respawnTime: ((victim: Entity, world: World) => number | undefined)[];
  /** false = l'entité ne peut pas se déplacer ce tick (root, stun, canalisation...). */
  canMove: ((e: Entity, world: World) => boolean)[];
  /** false = l'entité ne peut pas auto-attaquer (stun, désarmement, blind géré via beforeAttackHit...). */
  canAttack: ((e: Entity, world: World) => boolean)[];
  /** false = la cible n'est pas ciblable par `attacker` (invulnérable, stase, untargetable...). */
  isTargetable: ((target: Entity, attacker: Entity, world: World) => boolean)[];
  /** Avant l'impact d'une auto-attaque : modifier amount/crit/bonus (passifs d'AA, buff rouge, tours...). */
  beforeAttackHit: ((ctx: AttackContext, world: World) => void)[];
  /** Après l'impact d'une auto-attaque (on-hit : appliquer des buffs, cumuls...). */
  afterAttackHit: ((ctx: AttackContext, dealt: number, world: World) => void)[];
}

const emptyHooks = (): WorldHooks => ({
  beforeDamage: [],
  afterDamage: [],
  beforeHeal: [],
  preventDeath: [],
  bounty: [],
  respawnTime: [],
  canMove: [],
  canAttack: [],
  isTargetable: [],
  beforeAttackHit: [],
  afterAttackHit: [],
});

export interface System {
  name: string;
  /** Appelé une fois au démarrage de la partie (spawn des structures, abonnements EventBus...). */
  init?(world: World): void;
  update(world: World, dt: number): void;
}

export class World {
  entities = new Map<EntityId, Entity>();
  events = new EventBus();
  /** Commandes joueurs reçues depuis le dernier tick ; lisibles par tous les systèmes, vidées en fin de tick. */
  commands: QueuedCommand[] = [];
  /** Points d'extension (voir WorldHooks). */
  hooks: WorldHooks = emptyHooks();
  tick = 0;
  time = 0; // secondes de jeu
  winner: Team | null = null;
  private nextId = 1;
  private players = new Map<PlayerId, EntityId>();

  constructor(public systems: System[]) {}

  init(): void {
    for (const s of this.systems) s.init?.(this);
  }

  spawn(e: Omit<Entity, 'id' | 'data' | 'facing' | 'shield' | 'dead'> & Partial<Pick<Entity, 'data' | 'facing' | 'shield' | 'dead'>>): Entity {
    const ent: Entity = { facing: 0, shield: 0, dead: false, data: {}, ...e, id: this.nextId++ };
    this.entities.set(ent.id, ent);
    if (ent.champ) this.players.set(ent.champ.playerId, ent.id);
    return ent;
  }

  remove(id: EntityId): void {
    const e = this.entities.get(id);
    if (e?.champ && this.players.get(e.champ.playerId) === id) this.players.delete(e.champ.playerId);
    this.entities.delete(id);
  }

  /** Teste tous les hooks booléens : false si l'un d'eux refuse. */
  allow<A extends unknown[]>(fns: ((...args: [...A, World]) => boolean)[], ...args: A): boolean {
    for (const fn of fns) if (!fn(...args, this)) return false;
    return true;
  }

  get(id: EntityId | undefined): Entity | undefined {
    return id === undefined ? undefined : this.entities.get(id);
  }

  championOf(playerId: PlayerId): Entity | undefined {
    return this.get(this.players.get(playerId));
  }

  *query(kind?: EntityKind): IterableIterator<Entity> {
    for (const e of this.entities.values()) if (!kind || e.kind === kind) yield e;
  }

  step(dt = TICK_DT): void {
    this.tick++;
    this.time += dt;
    for (const s of this.systems) s.update(this, dt);
    this.commands = [];
  }
}

export function createChampion(
  world: World,
  opts: { playerId: PlayerId; name: string; team: Team; championId: string; summoners: [SummonerId, SummonerId]; pos: Vec2 },
): Entity {
  const def = getChampion(opts.championId);
  return world.spawn({
    kind: 'champion',
    team: opts.team,
    pos: { ...opts.pos },
    radius: 65,
    hp: def.baseStats.maxHp,
    mana: def.baseStats.maxMana,
    stats: { ...def.baseStats },
    subtype: def.id,
    anim: 'idle',
    champ: {
      playerId: opts.playerId,
      championId: def.id,
      name: opts.name,
      level: 1,
      xp: 0,
      skillPoints: 1,
      abilityLevels: { Q: 0, W: 0, E: 0, R: 0 },
      cooldowns: { Q: 0, W: 0, E: 0, R: 0, D: 0, F: 0 },
      summoners: { D: opts.summoners[0], F: opts.summoners[1] },
      gold: 500,
      items: [null, null, null, null, null, null],
      kills: 0,
      deaths: 0,
      assists: 0,
      cs: 0,
      respawnIn: 0,
      damageToChampions: 0,
    },
  });
}
