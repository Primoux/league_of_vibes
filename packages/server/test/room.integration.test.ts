/**
 * Test d'intégration M2 (claude-server, lead) : deux joueurs pilotés uniquement par des ClientMsg, à travers
 * Room + createSystems() (tous les systèmes réels présents sur main) + snapshots.
 * À compléter quand les sbires/tours de codex-objectives et l'économie de mistral-economy seront sur main.
 */
import { describe, expect, it } from 'vitest';
import type { ClientMsg, ServerMsg, Snapshot } from '@lov/shared';
import { addBuff } from '../src/combat/buffs.ts';
import { Room } from '../src/room.ts';

class Conn {
  msgs: ServerMsg[] = [];
  send(raw: string) {
    this.msgs.push(JSON.parse(raw) as ServerMsg);
  }
  snaps(): Snapshot[] {
    return this.msgs.filter((m): m is Extract<ServerMsg, { t: 'snapshot' }> => m.t === 'snapshot').map((m) => m.snap);
  }
}

function midLane() {
  let now = 0;
  const room = new Room('MIDXX', true, { clock: { now: () => now }, manual: true, random: () => 0 });
  const blue = new Conn();
  const red = new Conn();
  room.join('blue', 'Bleu', blue);
  room.join('red', 'Rouge', red);
  room.handle('blue', { t: 'startGame' });
  room.handle('blue', { t: 'draftLock', championId: 'kaelen' });
  room.handle('red', { t: 'draftLock', championId: 'sable' });
  for (let i = 0; i < 3; i++) {
    now += 10_000;
    room.update();
  }
  expect(room.phase).toBe('game');
  const world = room.world!;
  const send = (id: string, msg: ClientMsg) => room.handle(id, msg);
  const run = (s: number) => {
    for (let i = 0; i < Math.round(s * 30); i++) room.tickGame();
  };
  const b = world.championOf('blue')!;
  const r = world.championOf('red')!;
  // Place les deux champions au milieu de la voie mid.
  b.pos = { x: 5800, y: 5800 };
  r.pos = { x: 6300, y: 6300 };
  return { room, world, blue, red, b, r, send, run };
}

describe('intégration M2 — voie mid à deux joueurs', () => {
  it('commandes → déplacement, auto-attaques, sort, snapshots cohérents', () => {
    const { b, r, send, run, blue, red, world } = midLane();
    send('blue', { t: 'levelAbility', slot: 'Q' });
    run(0.1);
    expect(b.champ!.abilityLevels.Q).toBe(1);
    send('red', { t: 'attack', targetId: b.id });
    run(3);
    expect(b.hp).toBeLessThan(b.stats.maxHp); // Sable a poursuivi puis tiré
    send('blue', { t: 'attack', targetId: r.id }); // Kaelen (mêlée) engage
    run(1.5);
    send('blue', { t: 'cast', slot: 'Q', targetPos: { ...r.pos } });
    run(1);
    expect(r.hp).toBeLessThan(r.stats.maxHp);
    expect(blue.snaps().some((s) => s.events.some((e) => e.type === 'damage' && e.ability === 'kaelen_q'))).toBe(true);
    // Chaque joueur voit l'autre et a reçu les événements de combat.
    const bs = blue.snaps();
    const rs = red.snaps();
    expect(bs.length).toBeGreaterThan(70);
    const evTypes = new Set(bs.flatMap((s) => s.events.map((e) => e.type)));
    expect(evTypes).toContain('autoAttack');
    expect(evTypes).toContain('abilityCast');
    expect(evTypes).toContain('damage');
    expect(rs.some((s) => s.entities.some((e) => e.id === b.id))).toBe(true);
    expect(bs.at(-1)!.self!.abilityLevels.Q).toBe(1);
    expect(world.tick).toBeGreaterThan(100);
  });

  it('CC du système buffs : un champion étourdi ne bouge pas et n’attaque pas', () => {
    const { b, r, send, run, world } = midLane();
    addBuff(world, b, 'stun', { duration: 1, source: r.id });
    const start = { ...b.pos };
    send('blue', { t: 'move', pos: { x: 5000, y: 5000 } });
    run(0.5);
    expect(b.pos).toEqual(start);
    send('blue', { t: 'attack', targetId: r.id });
    const hp = r.hp;
    run(0.4);
    expect(r.hp).toBe(hp);
  });

  it('mort, kill crédité dans les snapshots, respawn, puis fin de partie', () => {
    const { b, r, send, run, blue, room } = midLane();
    r.hp = 30;
    send('blue', { t: 'attack', targetId: r.id });
    run(3);
    expect(r.dead).toBe(true);
    const snap = blue.snaps().at(-1)!;
    expect(snap.score.blue.kills).toBe(1);
    expect(blue.snaps().some((s) => s.events.some((e) => e.type === 'death' && e.target === r.id && e.killer === b.id))).toBe(true);
    run(r.champ!.respawnIn + 0.5);
    expect(r.dead).toBe(false);
    room.world!.winner = 'blue';
    run(0.1);
    expect(room.phase).toBe('end');
    const over = blue.msgs.find((m): m is Extract<ServerMsg, { t: 'gameOver' }> => m.t === 'gameOver')!.result;
    expect(over.stats.find((s) => s.playerId === 'blue')).toMatchObject({ kills: 1, championId: 'kaelen' });
    expect(over.stats.find((s) => s.playerId === 'red')).toMatchObject({ deaths: 1, championId: 'sable' });
  });
});
