import { describe, expect, it } from 'vitest';
import { Intensity, themeFor } from '../src/audio/music.ts';
import { MusicBus } from '../src/audio/music/instruments.ts';
import { defeat, gameBar, PHRASES, phraseInBounds, THEMES, victory } from '../src/audio/music/themes.ts';

/** Faux AudioContext : chaque nœud accepte connect/start/stop et des AudioParam inertes. */
function fakeCtx(): AudioContext {
  let created = 0;
  const param = () => ({ value: 0, setValueAtTime() {}, linearRampToValueAtTime() {}, exponentialRampToValueAtTime() {}, setTargetAtTime() {}, cancelScheduledValues() {} });
  const node = () => {
    created++;
    return new Proxy(
      { connect() {}, disconnect() {}, start() {}, stop() {}, buffer: null, loop: false, type: '' },
      { get: (o: Record<string, unknown>, k: string) => (k in o ? o[k] : (o[k] = param())) },
    );
  };
  const ctx = {
    currentTime: 0,
    sampleRate: 8000,
    get created() {
      return created;
    },
    createGain: node,
    createOscillator: node,
    createBiquadFilter: node,
    createDynamicsCompressor: node,
    createConvolver: node,
    createDelay: node,
    createChannelMerger: node,
    createBufferSource: node,
    createBuffer: (ch: number, len: number) => ({ getChannelData: () => new Float32Array(len), numberOfChannels: ch }),
  };
  return ctx as unknown as AudioContext;
}

describe('musique', () => {
  it('choisit le thème selon l’écran', () => {
    expect(themeFor({ game: null, result: null, draft: null, lobby: null })).toBe('menu');
    expect(themeFor({ game: null, result: null, draft: {} as never, lobby: { phase: 'draft' } as never })).toBe('draft');
    expect(themeFor({ game: {} as never, result: null, draft: null, lobby: null })).toBe('game');
    expect(themeFor({ game: {} as never, result: {} as never, draft: null, lobby: null })).toBe('end');
  });

  it('fait monter puis redescendre l’intensité de combat', () => {
    const i = new Intensity();
    i.bump(0.5);
    i.bump(0.8);
    expect(i.value).toBe(1);
    i.decay(10);
    expect(i.value).toBeCloseTo(0.4);
    i.floor = 0.3;
    i.decay(100);
    expect(i.value).toBe(0.3);
  });

  it('écrit des phrases dans les bornes de leur boucle', () => {
    expect(phraseInBounds(PHRASES.MENU_THEME, THEMES.menu.bars)).toBe(true);
    expect(phraseInBounds(PHRASES.DRAFT_COUNTER, THEMES.draft.bars)).toBe(true);
    expect(phraseInBounds(PHRASES.GAME_MELODY, THEMES.game.bars)).toBe(true);
  });

  it('joue chaque thème sur deux boucles, à toutes les intensités, sans erreur', () => {
    const ctx = fakeCtx();
    const bus = new MusicBus(ctx, ctx.createGain());
    for (const th of Object.values(THEMES))
      for (const intensity of [0, 0.3, 0.6, 1])
        for (let loop = 0; loop < 2; loop++) for (let step = 0; step < th.bars * 16; step++) th.play(bus, step, loop, step * 0.1, intensity, 0.1);
    victory(bus, 0);
    defeat(bus, 0);
    expect((ctx as unknown as { created: number }).created).toBeGreaterThan(1000);
  });

  it('varie le thème de partie d’une boucle à l’autre, en restant clairsemé', () => {
    const loops = new Set<string>();
    let bars = 0;
    let withMotif = 0;
    for (let loop = 0; loop < 12; loop++) {
      const plan = Array.from({ length: 8 }, (_, bar) => gameBar(loop, bar));
      loops.add(JSON.stringify(plan));
      for (const b of plan) {
        bars++;
        if (b.motif.length) withMotif++;
        for (const [at, , len] of b.motif) expect(at >= 0 && at < 16 && len > 0).toBe(true);
      }
      expect(JSON.stringify(plan)).toBe(JSON.stringify(Array.from({ length: 8 }, (_, bar) => gameBar(loop, bar))));
    }
    expect(loops.size).toBe(12);
    expect(withMotif / bars).toBeLessThan(0.45);
    expect(withMotif).toBeGreaterThan(0);
  });
});
