/**
 * Fabrique des séquences de GameEvent plausibles pour un sort, à partir de ses données (fxlab, tests).
 * C'est aussi la description de ce que le FX attend du serveur pour chaque type de ciblage.
 * Logique pure.
 */
import type { AbilityDef, AbilitySlot, ChampionDef, GameEvent, SummonerDef, Vec2 } from '@lov/shared';
import { clampToRange, towards } from '../telegraph/aim.ts';

export interface LabActor {
  id: number;
  pos: Vec2;
}

export interface TimedEvent {
  t: number;
  ev: GameEvent;
}

/** Passifs dont le déclenchement concerne la cible (givre, saignement, marque…) plutôt que le champion. */
const PASSIVE_ON_TARGET = new Set(['ilyra', 'vrenna', 'corvin', 'nyx']);

const dist = (a: Vec2, b: Vec2): number => Math.hypot(b.x - a.x, b.y - a.y);
const num = (v: number | number[] | undefined, fallback: number): number => (typeof v === 'number' ? v : Array.isArray(v) ? (v[0] ?? fallback) : fallback);

/** Événements d'un sort de champion lancé de `caster` vers `target`. `nextId` fournit les ids de projectiles. */
export function synthAbility(champ: ChampionDef, slot: AbilitySlot | 'P', caster: LabActor, target: LabActor, nextId: () => number): TimedEvent[] {
  const def: AbilityDef = slot === 'P' ? champ.passive : champ.abilities[slot];
  const out: TimedEvent[] = [];
  const key = def.fxKey;
  const p = def.params ?? {};

  if (slot === 'P') {
    // Convention : un passif qui se déclenche est signalé par un buff portant la fxKey du passif.
    const on = PASSIVE_ON_TARGET.has(champ.id) ? target.id : caster.id;
    const duration = num(p.stun, num(p.duration, 2));
    out.push({ t: 0, ev: { type: 'buffApplied', target: on, buffId: key, duration, source: caster.id } });
    return out;
  }

  const cast = def.castTime ?? 0;
  out.push({ t: 0, ev: { type: 'abilityCast', source: caster.id, championId: champ.id, slot, abilityId: def.id, targetPos: target.pos, targetId: target.id } });

  switch (def.targeting) {
    case 'skillshot':
    case 'global': {
      const speed = def.speed ?? 1500;
      const range = Math.min(def.range, 3000);
      const to = towards(caster.pos, target.pos, range);
      const id = nextId();
      out.push({ t: cast, ev: { type: 'projectileSpawn', id, source: caster.id, fxKey: key, from: caster.pos, to, speed, width: def.width ?? 80 } });
      const d = dist(caster.pos, target.pos);
      const hits = d <= range;
      const at = hits ? target.pos : to;
      out.push({ t: cast + (hits ? d : range) / speed, ev: { type: 'projectileHit', id, target: hits ? target.id : undefined, pos: at, fxKey: key } });
      break;
    }
    case 'cone': {
      const n = num(p.bullets, 5);
      const half = (num(p.coneAngle, 50) / 2) * (Math.PI / 180);
      const base = Math.atan2(target.pos.y - caster.pos.y, target.pos.x - caster.pos.x);
      const speed = def.speed ?? 2000;
      for (let i = 0; i < n; i++) {
        const a = base - half + (2 * half * i) / Math.max(1, n - 1);
        const to = { x: caster.pos.x + Math.cos(a) * def.range, y: caster.pos.y + Math.sin(a) * def.range };
        const id = nextId();
        out.push({ t: cast, ev: { type: 'projectileSpawn', id, source: caster.id, fxKey: key, from: caster.pos, to, speed, width: def.radius ?? 50 } });
        out.push({ t: cast + def.range / speed, ev: { type: 'projectileHit', id, pos: to, fxKey: key } });
      }
      break;
    }
    case 'ground': {
      const center = clampToRange(caster.pos, target.pos, def.range);
      const duration = num(p.delay, num(p.duration, 0.5));
      out.push({ t: cast, ev: { type: 'areaEffect', source: caster.id, fxKey: key, pos: center, radius: def.radius ?? 150, duration } });
      break;
    }
    case 'targeted': {
      // Sort ciblé : projectile rapide vers la cible si le sort est à distance, impact direct sinon.
      if (def.range > 400) {
        const id = nextId();
        const speed = def.speed ?? 1800;
        out.push({ t: cast, ev: { type: 'projectileSpawn', id, source: caster.id, fxKey: key, from: caster.pos, to: target.pos, speed, width: 60 } });
        out.push({ t: cast + dist(caster.pos, target.pos) / speed, ev: { type: 'projectileHit', id, target: target.id, pos: target.pos, fxKey: key } });
      } else {
        out.push({ t: cast + 0.1, ev: { type: 'projectileHit', id: nextId(), target: target.id, pos: target.pos, fxKey: key } });
      }
      break;
    }
    case 'dash':
    case 'targetedDash': {
      const to = def.targeting === 'targetedDash' ? towards(target.pos, caster.pos, 80) : clampToRange(caster.pos, target.pos, def.range);
      const speed = def.speed ?? 1200;
      const duration = Math.max(0.02, dist(caster.pos, to) / speed);
      out.push({ t: cast, ev: { type: 'dash', source: caster.id, from: caster.pos, to, duration, fxKey: key } });
      if (def.radius) {
        out.push({ t: cast + duration, ev: { type: 'areaEffect', source: caster.id, fxKey: key, pos: to, radius: def.radius, duration: 0 } });
      } else if (def.targeting === 'targetedDash' || def.damage) {
        out.push({ t: cast + duration, ev: { type: 'projectileHit', id: nextId(), target: target.id, pos: target.pos, fxKey: key } });
      }
      break;
    }
    case 'self': {
      if (def.radius) {
        const duration = num(p.duration, 0);
        out.push({ t: cast, ev: { type: 'areaEffect', source: caster.id, fxKey: key, pos: caster.pos, radius: def.radius, duration } });
      }
      const duration = num(p.duration, num(p.window, 0));
      if (duration > 0) out.push({ t: cast, ev: { type: 'buffApplied', target: caster.id, buffId: key, duration, source: caster.id } });
      break;
    }
  }
  return out.sort((a, b) => a.t - b.t);
}

/** Événements d'un sort d'invocateur. */
export function synthSummoner(def: SummonerDef, caster: LabActor, target: LabActor): TimedEvent[] {
  const to = def.id === 'flash' ? clampToRange(caster.pos, target.pos, def.range) : undefined;
  const out: TimedEvent[] = [
    { t: 0, ev: { type: 'summonerCast', source: caster.id, spell: def.id, from: caster.pos, to, targetId: def.targeting === 'targeted' ? target.id : undefined } },
  ];
  if (to) out.push({ t: 0, ev: { type: 'dash', source: caster.id, from: caster.pos, to, duration: 0.01, fxKey: def.fxKey } });
  return out;
}
