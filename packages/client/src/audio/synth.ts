/**
 * Synthèse WebAudio minimaliste (style zzfx, sans dépendance) : tout l'audio du jeu est
 * généré procéduralement, aucun asset externe (voir ASSETS.md). Propriétaire : mistral-economy.
 */

export interface ToneOpts {
  type?: OscillatorType;
  /** Fréquence de départ (Hz). */
  freq?: number;
  /** Glissement vers freq * slide. */
  slide?: number;
  dur?: number; // secondes
  vol?: number; // 0..1
  delay?: number; // secondes avant le début
  attack?: number;
  /** Fréquence de coupure d'un lowpass appliqué. */
  filter?: number;
}

let noiseBuffer: AudioBuffer | null = null;

function getNoise(ctx: AudioContext): AudioBuffer {
  if (!noiseBuffer || noiseBuffer.sampleRate !== ctx.sampleRate) {
    const len = ctx.sampleRate * 1.5;
    noiseBuffer = ctx.createBuffer(1, len, ctx.sampleRate);
    const data = noiseBuffer.getChannelData(0);
    for (let i = 0; i < len; i++) data[i] = Math.random() * 2 - 1;
  }
  return noiseBuffer;
}

/** Joue une onde (oscillateur) avec enveloppe ADSR-ish vers `dest`. */
export function tone(ctx: AudioContext, dest: AudioNode, o: ToneOpts): void {
  const t0 = ctx.currentTime + (o.delay ?? 0);
  const dur = o.dur ?? 0.15;
  const osc = ctx.createOscillator();
  osc.type = o.type ?? 'sine';
  const f0 = o.freq ?? 440;
  osc.frequency.setValueAtTime(f0, t0);
  if (o.slide && o.slide !== 1) osc.frequency.exponentialRampToValueAtTime(Math.max(20, f0 * o.slide), t0 + dur);
  const g = ctx.createGain();
  const vol = o.vol ?? 0.2;
  const atk = o.attack ?? 0.005;
  g.gain.setValueAtTime(0.0001, t0);
  g.gain.exponentialRampToValueAtTime(Math.max(0.0001, vol), t0 + atk);
  g.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
  let node: AudioNode = osc;
  if (o.filter) {
    const lp = ctx.createBiquadFilter();
    lp.type = 'lowpass';
    lp.frequency.value = o.filter;
    osc.connect(lp);
    node = lp;
  }
  node.connect(g);
  g.connect(dest);
  osc.start(t0);
  osc.stop(t0 + dur + 0.05);
}

/** Joue un bruit blanc filtré (percussions, explosions, whoosh). */
export function noise(ctx: AudioContext, dest: AudioNode, o: { dur?: number; vol?: number; delay?: number; filter?: number; slideFilter?: number; type?: BiquadFilterType }): void {
  const t0 = ctx.currentTime + (o.delay ?? 0);
  const dur = o.dur ?? 0.2;
  const src = ctx.createBufferSource();
  src.buffer = getNoise(ctx);
  src.loop = true;
  const g = ctx.createGain();
  const vol = o.vol ?? 0.2;
  g.gain.setValueAtTime(0.0001, t0);
  g.gain.exponentialRampToValueAtTime(Math.max(0.0001, vol), t0 + 0.01);
  g.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
  let node: AudioNode = src;
  if (o.filter) {
    const f = ctx.createBiquadFilter();
    f.type = o.type ?? 'lowpass';
    f.frequency.setValueAtTime(o.filter, t0);
    if (o.slideFilter) f.frequency.exponentialRampToValueAtTime(Math.max(40, o.slideFilter), t0 + dur);
    src.connect(f);
    node = f;
  }
  node.connect(g);
  g.connect(dest);
  src.start(t0);
  src.stop(t0 + dur + 0.05);
}
