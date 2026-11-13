import { describe, expect, it } from 'vitest';
import { BRUSHES } from '@lov/shared';
import { applyDamage } from '../src/combat/damage.ts';
import { isVisibleTo } from '../src/systems/vision.ts';
import { champ, coreWorld, dummy, run } from './helpers.ts';

const brushCenter = (i: number) => {
  const b = BRUSHES[i]!;
  return { x: b.reduce((s, p) => s + p.x, 0) / b.length, y: b.reduce((s, p) => s + p.y, 0) / b.length };
};

describe('vision', () => {
  it('rayon de vision des champions', () => {
    const w = coreWorld();
    champ(w, 'blue', { x: 6000, y: 3000 });
    const near = champ(w, 'red', { x: 6000, y: 4000 });
    const far = champ(w, 'red', { x: 6000, y: 5000 });
    w.step();
    expect(isVisibleTo(near, 'blue')).toBe(true);
    expect(isVisibleTo(far, 'blue')).toBe(false);
    expect(isVisibleTo(far, 'red')).toBe(true);
  });

  it('brousse : cachée sauf ennemi dans la même brousse ou balise', () => {
    const w = coreWorld();
    const c = brushCenter(0);
    const hidden = champ(w, 'red', c);
    const watcher = champ(w, 'blue', { x: c.x + 300, y: c.y });
    w.step();
    expect(isVisibleTo(hidden, 'blue')).toBe(false);
    watcher.pos = { ...c };
    w.step();
    expect(isVisibleTo(hidden, 'blue')).toBe(true);
    watcher.pos = { x: c.x + 3000, y: c.y };
    w.step();
    expect(isVisibleTo(hidden, 'blue')).toBe(false);
    dummy(w, 'blue', { x: c.x + 400, y: c.y }, {}, 'ward');
    w.step();
    expect(isVisibleTo(hidden, 'blue')).toBe(true);
  });

  it('attaquer depuis une brousse révèle 1 s', () => {
    const w = coreWorld();
    const c = brushCenter(0);
    const hidden = champ(w, 'red', c);
    const victim = champ(w, 'blue', { x: c.x + 400, y: c.y });
    w.step();
    expect(isVisibleTo(hidden, 'blue')).toBe(false);
    applyDamage(w, { source: hidden, target: victim, amount: 10, type: 'true' });
    w.step();
    expect(isVisibleTo(hidden, 'blue')).toBe(true);
    run(w, 1.2);
    expect(isVisibleTo(hidden, 'blue')).toBe(false);
  });

  it('invisibilité et révélation', () => {
    const w = coreWorld();
    champ(w, 'blue', { x: 6000, y: 3000 });
    const nyx = champ(w, 'red', { x: 6000, y: 3300 }, 'nyx');
    nyx.data.invisible = true;
    w.step();
    expect(isVisibleTo(nyx, 'blue')).toBe(false);
    nyx.data.revealedUntil = w.time + 1;
    w.step();
    expect(isVisibleTo(nyx, 'blue')).toBe(true);
  });

  it('on ne peut pas auto-attaquer une cible invisible', () => {
    const w = coreWorld();
    const a = champ(w, 'blue', { x: 6000, y: 3000 });
    const t = champ(w, 'red', { x: 6150, y: 3000 });
    t.data.invisible = true;
    w.step();
    a.attackTarget = t.id;
    run(w, 1);
    expect(t.hp).toBe(t.stats.maxHp);
  });
});
