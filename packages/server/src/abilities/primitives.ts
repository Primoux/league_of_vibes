import { MAP_SIZE, WALLS, clamp, dist, dist2, moveTowards, pointInPolygon, type Vec2 } from '@lov/shared';
import { canAct, hasCC } from '../combat/buffs.ts';
import { isVisibleTo } from '../systems/vision.ts';
import { movementSystem, type DashData } from '../systems/movement.ts';
import type { Entity, World } from '../world.ts';

export const isUnit = (e: Entity): boolean => ['champion', 'minion', 'monster'].includes(e.kind);
export const isEnemy = (source: Entity, target: Entity): boolean => !target.dead && target.hp > 0 && source.id !== target.id && source.team !== target.team && isUnit(target) && !target.data.stasis;
export function isVisibleTarget(world: World, source: Entity, target: Entity): boolean {
  if (source.team === target.team) return true;
  if (Number(target.data.revealedUntil ?? -Infinity) > world.time || target.data.revealed) return true;
  if (target.data.invisible) return false;
  return isVisibleTo(target, source.team);
}
export const validPoint = (p: Vec2 | undefined): p is Vec2 => !!p && Number.isFinite(p.x) && Number.isFinite(p.y);
export function direction(from: Vec2, to: Vec2, fallback = 0): Vec2 {
  const d = dist(from, to);
  return d < 1e-6 ? { x: Math.cos(fallback), y: Math.sin(fallback) } : { x: (to.x - from.x) / d, y: (to.y - from.y) / d };
}
export function endpoint(from: Vec2, to: Vec2, range: number): Vec2 {
  const p = moveTowards(from, to, range);
  return { x: clamp(p.x, 0, MAP_SIZE), y: clamp(p.y, 0, MAP_SIZE) };
}
export function enemiesInRadius(world: World, source: Entity, pos: Vec2, radius: number): Entity[] {
  const out: Entity[] = [];
  for (const e of world.entities.values()) if (isEnemy(source, e) && dist2(e.pos, pos) <= (radius + e.radius) ** 2) out.push(e);
  return out;
}
export function alliesInRadius(world: World, source: Entity, pos: Vec2, radius: number): Entity[] {
  const out: Entity[] = [];
  for (const e of world.query('champion')) if (!e.dead && e.team === source.team && dist2(e.pos, pos) <= (radius + e.radius) ** 2) out.push(e);
  return out;
}
/** Earliest intersection on a swept segment with a circle, including hits at the origin. */
export function segmentHit(from: Vec2, to: Vec2, pos: Vec2, radius: number): number | undefined {
  const dx = to.x - from.x, dy = to.y - from.y, ox = from.x - pos.x, oy = from.y - pos.y;
  const c = ox * ox + oy * oy - radius * radius;
  if (c <= 0) return 0;
  const a = dx * dx + dy * dy;
  if (a < 1e-12) return;
  const b = 2 * (ox * dx + oy * dy), discriminant = b * b - 4 * a * c;
  if (discriminant < 0) return;
  const t = (-b - Math.sqrt(discriminant)) / (2 * a);
  return t >= 0 && t <= 1 ? t : undefined;
}

interface Task { at: number; run: () => void; cancelled: boolean; owner?: Entity; cancelOnDeath?: boolean }
export interface ProjectileOptions {
  source: Entity; from?: Vec2; to: Vec2; speed: number; width: number; fxKey: string;
  pierce?: boolean; stopOnChampion?: boolean; onlyChampions?: boolean; target?: Entity; homingTo?: Entity;
  onHit: (target: Entity, projectile: Entity) => void; onEnd?: (pos: Vec2) => void;
}
interface Projectile { entity: Entity; options: ProjectileOptions; hit: Set<number>; end: Vec2; finished: boolean }
export interface ZoneOptions {
  source: Entity; pos: Vec2; radius: number; duration: number; interval?: number; fxKey: string;
  follow?: Entity; invisible?: boolean; onTick: (zone: Entity, elapsed: number) => void; onEnd?: () => void;
}
interface Zone { entity: Entity; options: ZoneOptions; start: number; next: number; expires: number }
export interface DashState extends DashData {
  from: Vec2; to: Vec2; speed: number; duration: number; remaining: number; fxKey: string;
  /** Integration: movement may own advancement by setting handlesDashes=true on its System. */
  unstoppable?: boolean;
}
interface Dash { source: Entity; data: DashState; previous: Vec2; hit: Set<number>; options: DashOptions }
export interface DashOptions {
  fxKey: string; speed: number; width?: number; stopOnFirst?: boolean; unstoppable?: boolean;
  crossWalls?: boolean; target?: Entity; onHit?: (target: Entity) => void; onEnd?: () => void;
}
interface Runtime { tasks: Task[]; projectiles: Map<number, Projectile>; zones: Map<number, Zone>; dashes: Map<number, Dash> }
const runtimes = new WeakMap<World, Runtime>();
function runtime(world: World): Runtime {
  let rt = runtimes.get(world);
  if (!rt) runtimes.set(world, rt = { tasks: [], projectiles: new Map(), zones: new Map(), dashes: new Map() });
  return rt;
}
export function schedule(world: World, delay: number, fn: () => void, owner?: Entity, cancelOnDeath = false): () => void {
  const task: Task = { at: world.time + Math.max(0, delay), run: fn, cancelled: false, owner, cancelOnDeath };
  runtime(world).tasks.push(task);
  return () => { task.cancelled = true; };
}
export function spawnProjectile(world: World, options: ProjectileOptions): Entity {
  const from = options.from ?? options.source.pos;
  const e = world.spawn({ kind: 'projectile', team: options.source.team, subtype: options.fxKey, pos: { ...from }, radius: options.width / 2, hp: 1, mana: 0, stats: options.source.stats, data: { ownerId: options.source.id } });
  const p: Projectile = { entity: e, options, end: { ...options.to }, hit: new Set(), finished: false };
  runtime(world).projectiles.set(e.id, p);
  e.data.abilityProjectile = p;
  world.events.emit({ type: 'projectileSpawn', id: e.id, source: options.source.id, fxKey: options.fxKey, from: { ...from }, to: { ...options.to }, speed: options.speed, width: options.width });
  return e;
}
export function spawnZone(world: World, options: ZoneOptions): Entity {
  const e = world.spawn({ kind: 'zone', team: options.source.team, subtype: options.fxKey, pos: { ...options.pos }, radius: options.radius, hp: 1, mana: 0, stats: options.source.stats, data: { ownerId: options.source.id, invisible: options.invisible ?? false } });
  runtime(world).zones.set(e.id, { entity: e, options, start: world.time, next: world.time + (options.interval ?? 1), expires: world.time + options.duration });
  world.events.emit({ type: 'areaEffect', source: options.source.id, fxKey: options.fxKey, pos: { ...e.pos }, radius: options.radius, duration: options.duration });
  return e;
}
export function removeZone(world: World, id: number): void {
  const zone = runtime(world).zones.get(id);
  if (!zone) return;
  runtime(world).zones.delete(id);
  world.remove(id);
  zone.options.onEnd?.();
}
export function delayedArea(world: World, source: Entity, pos: Vec2, radius: number, delay: number, fxKey: string, onImpact: () => void): void {
  world.events.emit({ type: 'areaEffect', source: source.id, pos: { ...pos }, radius, duration: delay, fxKey: `${fxKey}_telegraph` });
  schedule(world, delay, () => {
    world.events.emit({ type: 'areaEffect', source: source.id, pos: { ...pos }, radius, duration: .25, fxKey });
    onImpact();
  });
}
export function cone(world: World, source: Entity, toward: Vec2, range: number, angleDegrees: number, hit: (e: Entity) => void): void {
  const dir = direction(source.pos, toward, source.facing);
  const cos = Math.cos(angleDegrees * Math.PI / 360);
  for (const target of enemiesInRadius(world, source, source.pos, range)) {
    const d = dist(source.pos, target.pos), rel = direction(source.pos, target.pos);
    if (d <= target.radius || dir.x * rel.x + dir.y * rel.y >= cos - target.radius / Math.max(1, d)) hit(target);
  }
}
/** Radius-aware walkability, using exact polygon edges rather than a fixed grid. */
export function isWalkable(pos: Vec2, radius = 0): boolean {
  if (pos.x < radius || pos.y < radius || pos.x > MAP_SIZE - radius || pos.y > MAP_SIZE - radius) return false;
  for (const wall of WALLS) {
    if (pointInPolygon(pos, wall)) return false;
    for (let i = 0; i < wall.length; i++) {
      const a = wall[i]!, b = wall[(i + 1) % wall.length]!;
      const dx = b.x - a.x, dy = b.y - a.y, length2 = dx * dx + dy * dy;
      const t = length2 ? clamp(((pos.x - a.x) * dx + (pos.y - a.y) * dy) / length2, 0, 1) : 0;
      if (dist2(pos, { x: a.x + t * dx, y: a.y + t * dy }) < radius * radius) return false;
    }
  }
  return true;
}
/** Nearest valid landing inside the cast radius; can cross a wall, never land inside one. */
export function validLanding(from: Vec2, desired: Vec2, range: number, radius = 0): Vec2 {
  const point = endpoint(from, desired, range);
  if (isWalkable(point, radius)) return point;
  for (let r = 5; r <= range * 2 + radius; r += 5) {
    const count = Math.max(16, Math.ceil(2 * Math.PI * r / 10));
    for (let i = 0; i < count; i++) {
      const angle = i / count * 2 * Math.PI;
      const candidate = { x: point.x + Math.cos(angle) * r, y: point.y + Math.sin(angle) * r };
      if (dist2(candidate, from) <= range ** 2 + 1e-6 && isWalkable(candidate, radius)) return candidate;
    }
  }
  return { ...from };
}
export function blink(world: World, source: Entity, point: Vec2, range: number, fxKey: string): void {
  const from = { ...source.pos }, to = validLanding(from, point, range, source.radius);
  source.pos = to;
  source.moveTarget = undefined; source.path = undefined;
  world.events.emit({ type: 'dash', source: source.id, from, to: { ...to }, duration: 0, fxKey });
}
export function startDash(world: World, source: Entity, desired: Vec2, options: DashOptions): void {
  const from = { ...source.pos };
  let to = validLanding(from, desired, dist(from, desired), source.radius);
  if (!options.crossWalls) {
    const length = dist(from, to);
    for (let step = 10; step <= length; step += 10) {
      const point = moveTowards(from, to, step);
      if (!isWalkable(point, source.radius)) { to = moveTowards(from, to, Math.max(0, step - 10)); break; }
    }
  }
  const duration = dist(from, to) / Math.max(1, options.speed);
  const usesMovement = world.systems.includes(movementSystem);
  const data: DashState = { from, to, speed: options.speed, duration, remaining: duration, fxKey: options.fxKey, unstoppable: options.unstoppable,
    ignoreWalls: options.crossWalls, targetId: options.target?.id, stopDistance: options.target ? 0 : undefined };
  const dash: Dash = { source, data, options, previous: { ...from }, hit: new Set() };
  data.onArrive = () => { resolveDashHits(world, dash); finishDash(world, dash); };
  runtime(world).dashes.set(source.id, dash);
  source.data.dash = data;
  source.moveTarget = undefined; source.path = undefined;
  source.anim = 'dash';
  if (!usesMovement) world.events.emit({ type: 'dash', source: source.id, from, to: { ...to }, duration, fxKey: options.fxKey });
}
function finishDash(world: World, dash: Dash): void {
  if (!runtime(world).dashes.delete(dash.source.id)) return;
  if (dash.source.data.dash === dash.data) delete dash.source.data.dash;
  if (!dash.source.dead) dash.source.anim = 'idle';
  dash.options.onEnd?.();
}
function resolveDashHits(world: World, dash: Dash): void {
  if (!dash.options.onHit) return;
  const e = dash.source, from = dash.previous, to = { ...e.pos };
  const hits: { target: Entity; t: number }[] = [];
  for (const target of world.entities.values()) {
    if (!isEnemy(e, target) || dash.hit.has(target.id)) continue;
    const t = segmentHit(from, to, target.pos, (dash.options.width ?? e.radius * 2) / 2 + target.radius);
    if (t !== undefined) hits.push({ target, t });
  }
  hits.sort((a, b) => a.t - b.t || a.target.id - b.target.id);
  for (const { target, t } of hits) {
    dash.hit.add(target.id);
    if (dash.options.stopOnFirst) e.pos = { x: from.x + (to.x - from.x) * t, y: from.y + (to.y - from.y) * t };
    dash.options.onHit(target);
    if (dash.options.stopOnFirst) { finishDash(world, dash); break; }
  }
  dash.previous = { ...e.pos };
}

export function tether(world: World, source: Entity, target: Entity, duration: number, breakRange: number, onComplete: () => void): void {
  const start = world.time;
  const check = (): void => {
    if (source.dead || target.dead || !world.get(source.id) || !world.get(target.id) || dist2(source.pos, target.pos) > breakRange ** 2) return;
    if (world.time + 1e-8 >= start + duration) onComplete();
    else schedule(world, Math.min(1 / 30, start + duration - world.time), check);
  };
  schedule(world, Math.min(duration, 1 / 30), check);
}
function finishProjectile(world: World, rt: Runtime, p: Projectile): void {
  if (p.finished) return;
  p.finished = true;
  rt.projectiles.delete(p.entity.id); world.remove(p.entity.id);
  p.options.onEnd?.({ ...p.entity.pos });
}
export function updatePrimitives(world: World, dt: number): void {
  const rt = runtime(world);
  // Remove due tasks before invoking callbacks, so newly scheduled work survives this update.
  const due: Task[] = [];
  for (let i = rt.tasks.length - 1; i >= 0; i--) {
    const task = rt.tasks[i]!;
    if (task.cancelled || (task.cancelOnDeath && (task.owner?.dead || !world.get(task.owner?.id)))) rt.tasks.splice(i, 1);
    else if (task.at <= world.time + 1e-8) { rt.tasks.splice(i, 1); due.push(task); }
  }
  due.sort((a, b) => a.at - b.at);
  for (const task of due) if (!task.cancelled) task.run();
  // New return projectiles start next tick, never move twice during the same update.
  for (const p of [...rt.projectiles.values()]) {
    if (!world.get(p.entity.id)) { rt.projectiles.delete(p.entity.id); continue; }
    const { options: o, entity: e } = p;
    if (o.target) {
      if (o.target.dead || !world.get(o.target.id)) { finishProjectile(world, rt, p); continue; }
      p.end = { ...o.target.pos };
    }
    if (o.homingTo && world.get(o.homingTo.id)) p.end = { ...o.homingTo.pos };
    const from = e.pos, to = moveTowards(from, p.end, o.speed * dt);
    const hits: { target: Entity; t: number }[] = [];
    for (const target of world.entities.values()) {
      if (!isEnemy(o.source, target) || p.hit.has(target.id) || (o.onlyChampions && !target.champ) || (o.target && o.target.id !== target.id)) continue;
      const t = segmentHit(from, to, target.pos, o.width / 2 + target.radius);
      if (t !== undefined) hits.push({ target, t });
    }
    hits.sort((a, b) => a.t - b.t || a.target.id - b.target.id);
    let stopped = false;
    for (const { target, t } of hits) {
      p.hit.add(target.id);
      const pos = { x: from.x + (to.x - from.x) * t, y: from.y + (to.y - from.y) * t };
      world.events.emit({ type: 'projectileHit', id: e.id, target: target.id, pos, fxKey: o.fxKey });
      o.onHit(target, e);
      if ((!o.pierce && !o.stopOnChampion) || (o.stopOnChampion && !!target.champ)) { e.pos = pos; stopped = true; break; }
    }
    if (!stopped) e.pos = to;
    if (stopped || dist2(to, p.end) < 1e-6) finishProjectile(world, rt, p);
  }
  for (const zone of rt.zones.values()) {
    if (!world.get(zone.entity.id)) { rt.zones.delete(zone.entity.id); continue; }
    if (zone.options.follow) {
      if (zone.options.follow.dead) { removeZone(world, zone.entity.id); continue; }
      zone.entity.pos = { ...zone.options.follow.pos };
    }
    while (zone.next <= Math.min(world.time, zone.expires) + 1e-8 && rt.zones.has(zone.entity.id)) {
      zone.options.onTick(zone.entity, zone.next - zone.start);
      zone.next += Math.max(1 / 30, zone.options.interval ?? 1);
    }
    if (world.time + 1e-8 >= zone.expires) removeZone(world, zone.entity.id);
  }
  const movementOwnsDashes = world.systems.includes(movementSystem);
  for (const dash of [...rt.dashes.values()]) {
    const { source: e, data, options: o } = dash;
    if (e.dead || !world.get(e.id) || e.data.dash !== data || (!data.unstoppable && (!canAct(e) || hasCC(e, 'root')))) {
      rt.dashes.delete(e.id); if (e.data.dash === data) delete e.data.dash; if (!e.dead) e.anim = 'idle'; continue;
    }
    if (!movementOwnsDashes) {
      e.pos = moveTowards(e.pos, data.to, data.speed * dt); data.remaining = Math.max(0, data.remaining - dt);
      resolveDashHits(world, dash);
      if (data.remaining <= 1e-8 || dist2(e.pos, data.to) < 1) finishDash(world, dash);
    } else resolveDashHits(world, dash);
  }
}
