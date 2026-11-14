import { describe, expect, it } from 'vitest';
import { applyEntityPatch, type EntitySnapshot, type Snapshot } from '@lov/shared';
import { applyDamage, applyShield, removeShield } from '../src/combat/damage.ts';
import { ClientView, REFRESH_TICKS, ScoreTracker, SnapshotFrame, buildSnapshot } from '../src/net/snapshot.ts';
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
    expect(s2.entities).toHaveLength(0);
    expect(s2.patches).toEqual([{ id: foe.id, hp: Math.round(foe.hp) }]);
    foe.pos = { x: 10000, y: 10000 };
    w.step();
    const s3 = view.build(new SnapshotFrame(w, [], score.score()), me.champ!.playerId, 'blue');
    expect(s3.removed).toEqual([foe.id]);
    expect(s3.entities).toHaveLength(0);
  });

  it('patches : la vue reconstruite côté client = le snapshot complet (champs ajoutés, modifiés, supprimés)', () => {
    const w = coreWorld();
    const me = champ(w, 'blue', { x: 6000, y: 3000 });
    const foe = champ(w, 'red', { x: 6000, y: 3500 });
    const score = new ScoreTracker(w);
    const view = new ClientView();
    const client = new Map<number, EntitySnapshot>();
    const apply = (s: Snapshot) => {
      for (const id of s.removed ?? []) client.delete(id);
      for (const e of s.entities) client.set(e.id, e);
      for (const p of s.patches ?? []) client.set(p.id, applyEntityPatch(client.get(p.id)!, p));
    };
    const mutations = [
      () => applyShield(w, { source: foe, target: foe, amount: 50 }), // champ ajouté (shield)
      () => (foe.champ!.kills = 2), // objet imbriqué modifié (champ)
      () => removeShield(foe), // champ supprimé (shield)
      () => (foe.pos = { x: 6100, y: 3400 }),
      () => applyDamage(w, { source: me, target: foe, amount: 1e6, type: 'true' }), // dead + respawnIn
      () => {},
    ];
    for (const m of mutations) {
      m();
      w.step();
      const frame = new SnapshotFrame(w, w.events.drain(), score.score());
      apply(view.build(frame, me.champ!.playerId, 'blue'));
      const truth = new ClientView().build(frame, me.champ!.playerId, 'blue');
      expect(Object.fromEntries(client)).toEqual(Object.fromEntries(truth.entities.map((e) => [e.id, e])));
    }
  });

  it("rafraîchissement étalé : une entité dont le client a perdu l'état revient complète en ≤ REFRESH_TICKS", () => {
    const w = coreWorld();
    const me = champ(w, 'blue', { x: 6000, y: 3000 });
    const foe = champ(w, 'red', { x: 6000, y: 3500 });
    const score = new ScoreTracker(w);
    const view = new ClientView();
    const seenFull = new Set<number>();
    for (let i = 0; i < REFRESH_TICKS; i++) {
      w.step();
      const s = view.build(new SnapshotFrame(w, [], score.score()), me.champ!.playerId, 'blue');
      if (i > 0) for (const e of s.entities) seenFull.add(e.id);
    }
    expect(seenFull).toEqual(new Set([me.id, foe.id]));
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
