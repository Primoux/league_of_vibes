/**
 * Validation des messages client et limitation de débit. Propriétaire : claude-server.
 * Tout message entrant passe par `parseClientMsg` : forme vérifiée, nombres finis, chaînes bornées.
 */
import { MAP_SIZE, SUMMONER_IDS, type ClientMsg, type Vec2 } from '@lov/shared';

const TEAMS = new Set(['blue', 'red']);
const ROLES = new Set(['top', 'jungle', 'mid', 'bot']);
const SLOTS = new Set(['Q', 'W', 'E', 'R']);
const SUM_SLOTS = new Set(['D', 'F']);
const SUMS = new Set<string>(SUMMONER_IDS);
const PING_KIND = /^[a-z_]{1,16}$/;

type Obj = Record<string, unknown>;

const isObj = (v: unknown): v is Obj => typeof v === 'object' && v !== null && !Array.isArray(v);
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const str = (v: unknown, max: number): v is string => typeof v === 'string' && v.length <= max;
const id = (v: unknown): v is number => Number.isInteger(v) && (v as number) > 0;
const pos = (v: unknown): v is Vec2 =>
  isObj(v) && num(v.x) && num(v.y) && v.x >= -1000 && v.y >= -1000 && v.x <= MAP_SIZE + 1000 && v.y <= MAP_SIZE + 1000;
const optPos = (v: unknown): boolean => v === undefined || pos(v);
const optId = (v: unknown): boolean => v === undefined || id(v);

/** Parse et valide un message brut. Retourne null si invalide. */
export function parseClientMsg(raw: string): ClientMsg | null {
  if (raw.length > 4096) return null;
  let m: unknown;
  try {
    m = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!isObj(m) || typeof m.t !== 'string') return null;
  return validate(m) ? (m as unknown as ClientMsg) : null;
}

function validate(m: Obj): boolean {
  switch (m.t) {
    case 'hello':
      return str(m.name, 64) && num(m.version) && (m.reconnectToken === undefined || str(m.reconnectToken, 128));
    case 'createRoom':
      return m.devMode === undefined || typeof m.devMode === 'boolean';
    case 'joinRoom':
      return str(m.code, 16);
    case 'leaveRoom':
    case 'startGame':
    case 'stop':
    case 'undoShop':
    case 'recall':
    case 'returnToLobby':
      return true;
    case 'setTeam':
      return TEAMS.has(m.team as string);
    case 'setRole':
      return ROLES.has(m.role as string);
    case 'setReady':
      return typeof m.ready === 'boolean';
    case 'draftHover':
    case 'draftLock':
      return str(m.championId, 32);
    case 'draftSummoners':
      return Array.isArray(m.summoners) && m.summoners.length === 2 && m.summoners.every((s) => SUMS.has(s as string));
    case 'move':
    case 'attackMove':
    case 'placeWard':
      return pos(m.pos);
    case 'attack':
      return id(m.targetId);
    case 'cast':
      return SLOTS.has(m.slot as string) && optPos(m.targetPos) && optId(m.targetId);
    case 'summoner':
      return SUM_SLOTS.has(m.slot as string) && optPos(m.targetPos) && optId(m.targetId);
    case 'levelAbility':
      return SLOTS.has(m.slot as string);
    case 'buy':
      return str(m.itemId, 48);
    case 'useItem':
      // slot −1 = trinket, 0..5 = inventaire (protocol.ts)
      return Number.isInteger(m.slot) && (m.slot as number) >= -1 && (m.slot as number) <= 5 && optPos(m.targetPos);
    case 'sell':
      return Number.isInteger(m.slot) && (m.slot as number) >= 0 && (m.slot as number) < 8;
    case 'ping':
      return pos(m.pos) && typeof m.kind === 'string' && PING_KIND.test(m.kind);
    case 'chat':
      return str(m.text, 200) && (m.text as string).trim().length > 0 && typeof m.teamOnly === 'boolean';
    case 'netPing':
      return num(m.clientTime);
    default:
      return false;
  }
}

/**
 * Seau à jetons : `rate` messages/s en moyenne, rafales jusqu'à `burst`.
 * `take()` retourne false si le message doit être ignoré ; `strikes` compte les dépassements.
 */
export class RateLimiter {
  private tokens: number;
  private last: number;
  strikes = 0;

  constructor(
    private rate: number,
    private burst: number,
    private now: () => number = Date.now,
  ) {
    this.tokens = burst;
    this.last = now();
  }

  take(cost = 1): boolean {
    const t = this.now();
    this.tokens = Math.min(this.burst, this.tokens + ((t - this.last) / 1000) * this.rate);
    this.last = t;
    if (this.tokens < cost) {
      this.strikes++;
      return false;
    }
    this.tokens -= cost;
    return true;
  }
}
