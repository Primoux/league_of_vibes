/**
 * Simulation du fxlab : fait tourner la VRAIE simulation serveur (World + systèmes de combat de claude-server
 * et codex-champions, importés en lecture seule) dans le navigateur. Les événements produits sont exactement
 * ceux du jeu ; ils sont publiés sur le fxBus comme le ferait net/client.
 *
 * Systèmes inclus : input, buffs, invocateurs, sorts, mouvement, combat, progression (pas de vagues, jungle,
 * économie ni vision : le lab ne garde que les champions).
 */
import { SUMMONERS, TICK_DT, type AbilitySlot, type EntitySnapshot, type GameEvent, type SummonerId, type Team, type Vec2 } from '@lov/shared';
import { applyDamage } from '../../../../server/src/combat/damage.ts';
import { toEntitySnapshot } from '../../../../server/src/net/snapshot.ts';
import { abilitiesSystem } from '../../../../server/src/systems/abilities.ts';
import { buffsSystem } from '../../../../server/src/systems/buffs.ts';
import { combatSystem } from '../../../../server/src/systems/combat.ts';
import { inputSystem } from '../../../../server/src/systems/input.ts';
import { levelingSystem } from '../../../../server/src/systems/leveling.ts';
import { movementSystem } from '../../../../server/src/systems/movement.ts';
import { summonersSystem } from '../../../../server/src/systems/summoners.ts';
import { World, createChampion, type Entity } from '../../../../server/src/world.ts';

export interface SimActor {
  id: number;
  playerId: string;
  team: Team;
  home: Vec2;
  /** Position/orientation lissées entre deux ticks (30 Hz → affichage). */
  x: number;
  y: number;
  facing: number;
  prevX: number;
  prevY: number;
  /** Le lab maintient la cible en vie, sauf si on la tue explicitement. */
  immortal: boolean;
}

export class LabSim {
  readonly world = new World([inputSystem, buffsSystem, summonersSystem, abilitiesSystem, movementSystem, combatSystem, levelingSystem]);
  readonly actors = new Map<number, SimActor>();
  private acc = 0;
  private seq = 0;

  constructor(private readonly publish: (e: GameEvent) => void) {
    this.world.init();
  }

  /** Crée un champion de niveau 18, tous sorts au rang max. */
  spawn(championId: string, team: Team, pos: Vec2): SimActor {
    const playerId = `lab${this.seq++}`;
    const e = createChampion(this.world, { playerId, name: playerId, team, championId, summoners: ['flash', 'ignite'], pos });
    const c = e.champ!;
    c.level = 18;
    c.abilityLevels = { Q: 5, W: 5, E: 5, R: 3 };
    c.skillPoints = 0;
    const a: SimActor = { id: e.id, playerId, team, home: { ...pos }, x: pos.x, y: pos.y, facing: 0, prevX: pos.x, prevY: pos.y, immortal: true };
    this.actors.set(e.id, a);
    return a;
  }

  remove(id: number): void {
    this.world.remove(id);
    this.actors.delete(id);
  }

  entity(id: number): Entity | undefined {
    return this.world.get(id);
  }

  snapshot(id: number): EntitySnapshot | null {
    const e = this.world.get(id);
    return e ? toEntitySnapshot(e, this.world) : null;
  }

  private refresh(e: Entity): void {
    // ressources et délais remis à zéro : le lab teste les FX, pas l'économie de mana
    e.mana = e.stats.maxMana;
    const cd = e.champ!.cooldowns;
    cd.Q = cd.W = cd.E = cd.R = cd.D = cd.F = 0;
    // invocateurs : fenêtre de relance éventuelle remise à zéro
  }

  cast(casterId: number, slot: AbilitySlot, target: SimActor | null, point: Vec2): void {
    const e = this.world.get(casterId);
    if (!e?.champ) return;
    this.refresh(e);
    this.world.commands.push({ playerId: e.champ.playerId, msg: { t: 'cast', slot, targetPos: { ...point }, targetId: target?.id } });
  }

  summoner(casterId: number, spell: SummonerId, target: SimActor | null, point: Vec2): void {
    const e = this.world.get(casterId);
    if (!e?.champ) return;
    this.refresh(e);
    e.champ.summoners.D = spell;
    const def = SUMMONERS[spell];
    const targetId = def.targeting === 'targeted' ? target?.id : undefined;
    this.world.commands.push({ playerId: e.champ.playerId, msg: { t: 'summoner', slot: 'D', targetPos: { ...point }, targetId } });
  }

  attack(casterId: number, targetId: number): void {
    const e = this.world.get(casterId);
    if (e?.champ) this.world.commands.push({ playerId: e.champ.playerId, msg: { t: 'attack', targetId } });
  }

  stop(casterId: number): void {
    const e = this.world.get(casterId);
    if (e?.champ) this.world.commands.push({ playerId: e.champ.playerId, msg: { t: 'stop' } });
  }

  moveTo(id: number, pos: Vec2): void {
    const e = this.world.get(id);
    if (e?.champ) this.world.commands.push({ playerId: e.champ.playerId, msg: { t: 'move', pos: { ...pos } } });
  }

  kill(targetId: number, killerId: number): void {
    const t = this.world.get(targetId);
    const k = this.world.get(killerId);
    const a = this.actors.get(targetId);
    if (!t || !k || t.dead) return;
    if (a) a.immortal = false;
    applyDamage(this.world, { source: k, target: t, amount: 1e6, type: 'true' });
    if (t.champ) t.champ.respawnIn = 3;
  }

  /** Remet tout le monde à sa place et en pleine santé. */
  reset(): void {
    for (const a of this.actors.values()) {
      const e = this.world.get(a.id);
      if (!e) continue;
      e.pos = { ...a.home };
      e.moveTarget = undefined;
      e.path = undefined;
      e.attackTarget = undefined;
      e.hp = e.stats.maxHp;
      a.prevX = a.x = e.pos.x;
      a.prevY = a.y = e.pos.y;
    }
  }

  /** Avance la simulation (ticks de 1/30 s) et publie les événements. */
  update(dt: number): void {
    this.acc += dt;
    while (this.acc >= TICK_DT) {
      this.acc -= TICK_DT;
      for (const a of this.actors.values()) {
        const e = this.world.get(a.id);
        if (!e) continue;
        a.prevX = e.pos.x;
        a.prevY = e.pos.y;
      }
      this.world.step();
      for (const ev of this.world.events.drain()) this.publish(ev);
      for (const a of this.actors.values()) {
        const e = this.world.get(a.id);
        if (!e) continue;
        if (a.immortal && !e.dead && e.hp < e.stats.maxHp * 0.35) e.hp = e.stats.maxHp;
        if (!e.dead) a.immortal = true;
      }
    }
    // interpolation entre les deux derniers ticks
    const k = this.acc / TICK_DT;
    for (const a of this.actors.values()) {
      const e = this.world.get(a.id);
      if (!e) continue;
      a.x = a.prevX + (e.pos.x - a.prevX) * k;
      a.y = a.prevY + (e.pos.y - a.prevY) * k;
      a.facing = e.facing;
    }
  }
}
