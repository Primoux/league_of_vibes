import { describe, expect, it } from 'vitest';
import { KillStreaks, stingLevel } from '../src/audio/killStreaks.ts';
import { MusicBus } from '../src/audio/music/instruments.ts';
import { firstBloodSting, killSting } from '../src/audio/music/stings.ts';

describe('séries de kills (champions uniquement)', () => {
  it('premier sang, puis multi-kill dans la fenêtre de 10 s', () => {
    const k = new KillStreaks();
    expect(k.onChampionKill(10, 1, 0)).toEqual({ multi: 1, spree: 1, firstBlood: true });
    expect(k.onChampionKill(11, 1, 4)).toEqual({ multi: 2, spree: 2, firstBlood: false });
    expect(k.onChampionKill(12, 1, 9)).toMatchObject({ multi: 3, spree: 3 });
    expect(k.onChampionKill(13, 1, 30)).toMatchObject({ multi: 1, spree: 4 }); // hors fenêtre : série continue
  });

  it('la mort du tueur remet sa série à zéro, même exécuté par une tour', () => {
    const k = new KillStreaks();
    k.onChampionKill(10, 1, 0);
    k.onChampionKill(11, 1, 20);
    k.onChampionDeath(1);
    expect(k.onChampionKill(12, 1, 60)).toMatchObject({ multi: 1, spree: 1 });
  });

  it('le jingle monte en puissance avec le multi-kill et la série', () => {
    const lvl = (multi: number, spree: number) => stingLevel({ multi, spree, firstBlood: false });
    expect(lvl(1, 1)).toBe(0);
    expect(lvl(1, 3)).toBe(1);
    expect(lvl(2, 2)).toBe(2);
    expect(lvl(3, 3)).toBe(5);
    expect(lvl(5, 5)).toBe(9);
    // strictement croissant sur un massacre
    const seq = [1, 2, 3, 4, 5].map((n) => lvl(n, n));
    for (let i = 1; i < seq.length; i++) expect(seq[i]!).toBeGreaterThan(seq[i - 1]!);
  });

  it('chaque niveau de jingle se programme sans erreur', () => {
    const param = () => ({ value: 0, setValueAtTime() {}, linearRampToValueAtTime() {}, exponentialRampToValueAtTime() {}, setTargetAtTime() {}, cancelScheduledValues() {} });
    const node = () => new Proxy({ connect() {}, disconnect() {}, start() {}, stop() {} } as Record<string, unknown>, { get: (o, k: string) => (k in o ? o[k] : (o[k] = param())) });
    const ctx = { currentTime: 0, sampleRate: 8000, createGain: node, createOscillator: node, createBiquadFilter: node, createDynamicsCompressor: node, createConvolver: node, createDelay: node, createChannelMerger: node, createBufferSource: node, createBuffer: (_c: number, len: number) => ({ getChannelData: () => new Float32Array(len) }) } as unknown as AudioContext;
    const bus = new MusicBus(ctx, ctx.createGain());
    for (let l = 0; l <= 9; l++) for (const side of ['self', 'ally', 'enemy'] as const) killSting(bus, 0, l, side);
    firstBloodSting(bus, 0, 'ally');
    firstBloodSting(bus, 0, 'enemy');
  });
});

import { playEvent, resetKillAnnouncements } from '../src/audio/sounds.ts';
import { store } from '../src/state.ts';
import type { AudioManager } from '../src/audio/AudioManager.ts';

describe('événements death -> annonce sonore', () => {
  it('ignore les sbires, joue un jingle pour un kill entre champions', () => {
    const param = () => ({ value: 0, setValueAtTime() {}, linearRampToValueAtTime() {}, exponentialRampToValueAtTime() {}, setTargetAtTime() {}, cancelScheduledValues() {} });
    const node = () => new Proxy({ connect() {}, disconnect() {}, start() {}, stop() {} } as Record<string, unknown>, { get: (o, k: string) => (k in o ? o[k] : (o[k] = param())) });
    const ctx = { currentTime: 0, sampleRate: 8000, createGain: node, createOscillator: node, createBiquadFilter: node, createDynamicsCompressor: node, createConvolver: node, createDelay: node, createChannelMerger: node, createBufferSource: node, createBuffer: (_c: number, len: number) => ({ getChannelData: () => new Float32Array(len) }) } as unknown as AudioContext;
    const channels: string[] = [];
    const a = {
      sfx: (fn: (c: AudioContext, d: AudioNode) => void, o: { channel?: string } = {}) => {
        channels.push(o.channel ?? 'sfx');
        fn(ctx, ctx.createGain());
      },
      getVolume: () => 1,
      entityPos: () => undefined,
    } as unknown as AudioManager;
    const ent = (id: number, kind: string, team: string) => ({ id, kind, team, x: 0, y: 0, facing: 0, hp: 0, maxHp: 1 });
    store.set({
      game: { you: 'me', team: 'blue', entityId: 1, players: [], serverTime: 0 },
      snapshot: { tick: 1, time: 100, serverTime: 0, events: [], score: { blue: { kills: 0, towers: 0, dragons: [], grubs: 0 }, red: { kills: 0, towers: 0, dragons: [], grubs: 0 } }, entities: [ent(1, 'champion', 'blue'), ent(10, 'champion', 'red'), ent(50, 'minion', 'red'), ent(51, 'minion', 'red')] as never },
    });
    resetKillAnnouncements();
    playEvent(a, { type: 'death', target: 50, killer: 1 });
    playEvent(a, { type: 'death', target: 51, killer: 1 });
    expect(channels.filter((c) => c === 'voice')).toHaveLength(0);
    playEvent(a, { type: 'death', target: 10, killer: 1 });
    expect(channels.filter((c) => c === 'voice').length).toBeGreaterThan(0);
  });
});
