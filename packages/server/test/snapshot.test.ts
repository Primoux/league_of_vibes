import { describe, expect, it } from 'vitest';
import { applyDamage } from '../src/combat/damage.ts';
import { ClientView, ScoreTracker, SnapshotFrame, buildSnapshot } from '../src/net/snapshot.ts';
import { champ, coreWorld, dummy } from './helpers.ts';

describe('snapshots', () => {
  it("ne transmet pas les ennemis invisibles mais toujours les structures", () => {
    const w = coreWorld();
    const me = champ(w, 'blue', { x: 6000, y: 3000 });
    const near = champ(w, 'red', { x: 6000, y: 3500 });
    const far = champ(w, 'red', { x: 9000, y: 9000 });
    const tower = dummy(w, 'red', { x: 10000, y: 10000 }, {}, 'tower');
    w.step();
    const snap = buildSnapshot(w, me.champ!.playerId, 'blue', []);
    const ids = snap.entities.map((e) => e.id);
    expect(ids).toContain(me.id);
    expect(ids).toContain(near.id);
    expect(ids).toContain(tower.id);
    expect(ids).not.toContain(far.id);
  });

  it("filtre les événements d'unités invisibles", () => {
    const w = coreWorld();
    const me = champ(w, 'blue', { x: 6000, y: 3000 });
    const far = champ(w, 'red', { x: 9000, y: 9000 });
    const farMinion = dummy(w, 'blue', { x: 9100, y: 9000 });
    farMinion.data.sightRadius = 0; // ne donne pas la vision : seul le fait d'être la cible compte
    w.step();
    w.events.drain();
    applyDamage(w, { source: far, target: dummy(w, 'neutral', { x: 9000, y: 9100 }), amount: 10, type: 'true' });
    w.events.emit({ type: 'goldGain', target: far.id, amount: 10 });
    w.events.emit({ type: 'ping', playerId: far.champ!.playerId, pos: { x: 0, y: 0 }, kind: 'generic' });
    w.events.emit({ type: 'ping', playerId: me.champ!.playerId, pos: { x: 0, y: 0 }, kind: 'generic' });
    applyDamage(w, { source: far, target: farMinion, amount: 10, type: 'true' });
    const evs = buildSnapshot(w, me.champ!.playerId, 'blue', w.events.drain()).events;
    expect(evs.map((e) => e.type)).toEqual(['ping', 'damage']);
  });

  it('delta : entités modifiées seulement, removed quand une unité sort de la vision', () => {
    const w = coreWorld();
    const me = champ(w, 'blue', { x: 6000, y: 3000 });
    const foe = champ(w, 'red', { x: 6000, y: 3500 });
    const score = new ScoreTracker(w);
    const view = new ClientView();
    w.step();
    const s1 = view.build(new SnapshotFrame(w, [], score.score()), me.champ!.playerId, 'blue');
    expect(s1.removed).toBeUndefined();
    expect(s1.entities).toHaveLength(2);
    foe.hp -= 100;
    w.step();
    const s2 = view.build(new SnapshotFrame(w, [], score.score()), me.champ!.playerId, 'blue');
    expect(s2.entities.map((e) => e.id)).toEqual([foe.id]);
    foe.pos = { x: 10000, y: 10000 };
    w.step();
    const s3 = view.build(new SnapshotFrame(w, [], score.score()), me.champ!.playerId, 'blue');
    expect(s3.removed).toEqual([foe.id]);
    expect(s3.entities).toHaveLength(0);
  });

  it('score : kills et tours', () => {
    const w = coreWorld();
    const a = champ(w, 'blue', { x: 6000, y: 3000 });
    const tracker = new ScoreTracker(w);
    a.champ!.kills = 3;
    w.events.emit({ type: 'structureDestroyed', target: 99, kind: 'tower', team: 'red' });
    w.events.emit({ type: 'structureDestroyed', target: 99, kind: 'tower', team: 'red', plate: true });
    w.events.emit({ type: 'objectiveTaken', monster: 'dragon', team: 'blue', element: 'ocean' });
    expect(tracker.score().blue).toEqual({ kills: 3, towers: 1, dragons: ['ocean'], grubs: 0 });
  });
});
