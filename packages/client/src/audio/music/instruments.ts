/**
 * Instruments de la bande-son, 100 % synthétisés en WebAudio (aucun échantillon, voir ASSETS.md).
 * Chaque fonction programme des nœuds à l'instant `t` (temps du contexte) vers le bus de musique.
 */

export const midi = (n: number): number => 440 * Math.pow(2, (n - 69) / 12);

/** Bus de mixage : sec + réverbération de salle générée + délai stéréo, compressé. */
export class MusicBus {
  readonly dry: GainNode;
  readonly verb: GainNode; // envoi vers la réverbération
  readonly echo: GainNode; // envoi vers le délai
  readonly master: GainNode; // fondus entre thèmes
  private noiseBuf: AudioBuffer;

  constructor(
    readonly ctx: AudioContext,
    dest: AudioNode,
  ) {
    this.master = ctx.createGain();
    this.master.gain.value = 0;
    const comp = ctx.createDynamicsCompressor();
    comp.threshold.value = -16;
    comp.knee.value = 12;
    comp.ratio.value = 3.5;
    comp.attack.value = 0.01;
    comp.release.value = 0.3;
    this.master.connect(comp);
    comp.connect(dest);

    this.dry = ctx.createGain();
    this.dry.connect(this.master);

    // réverbération : réponse impulsionnelle stéréo générée (bruit à décroissance exponentielle, 3,2 s)
    const conv = ctx.createConvolver();
    conv.buffer = makeImpulse(ctx, 3.2, 2.6);
    const verbOut = ctx.createGain();
    verbOut.gain.value = 0.55;
    this.verb = ctx.createGain();
    this.verb.connect(conv);
    conv.connect(verbOut);
    verbOut.connect(this.master);

    // délai ping-pong (croche pointée réglée par le thème)
    this.echo = ctx.createGain();
    const dl = ctx.createDelay(2);
    const dr = ctx.createDelay(2);
    dl.delayTime.value = 0.375;
    dr.delayTime.value = 0.375;
    const fb = ctx.createGain();
    fb.gain.value = 0.32;
    const tone = ctx.createBiquadFilter();
    tone.type = 'lowpass';
    tone.frequency.value = 2600;
    const merger = ctx.createChannelMerger(2);
    this.echo.connect(dl);
    dl.connect(tone);
    tone.connect(dr);
    dr.connect(fb);
    fb.connect(dl);
    dl.connect(merger, 0, 0);
    dr.connect(merger, 0, 1);
    const echoOut = ctx.createGain();
    echoOut.gain.value = 0.35;
    merger.connect(echoOut);
    echoOut.connect(this.master);
    this.delays = [dl, dr];

    this.noiseBuf = ctx.createBuffer(1, ctx.sampleRate * 2, ctx.sampleRate);
    const d = this.noiseBuf.getChannelData(0);
    for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
  }

  private delays: DelayNode[];

  setDelay(seconds: number): void {
    for (const d of this.delays) d.delayTime.setTargetAtTime(seconds, this.ctx.currentTime, 0.1);
  }

  /** Fondu du volume global de la musique. */
  fade(to: number, seconds: number): void {
    const g = this.master.gain;
    const now = this.ctx.currentTime;
    g.cancelScheduledValues(now);
    g.setValueAtTime(g.value, now);
    g.linearRampToValueAtTime(to, now + seconds);
  }

  /** Sortie d'une voix : sec + envois réverbération / délai. */
  voice(verb = 0.3, echo = 0): GainNode {
    const g = this.ctx.createGain();
    g.connect(this.dry);
    if (verb > 0) {
      const s = this.ctx.createGain();
      s.gain.value = verb;
      g.connect(s);
      s.connect(this.verb);
    }
    if (echo > 0) {
      const s = this.ctx.createGain();
      s.gain.value = echo;
      g.connect(s);
      s.connect(this.echo);
    }
    return g;
  }

  noise(): AudioBufferSourceNode {
    const src = this.ctx.createBufferSource();
    src.buffer = this.noiseBuf;
    src.loop = true;
    return src;
  }
}

function makeImpulse(ctx: AudioContext, seconds: number, decay: number): AudioBuffer {
  const len = Math.floor(ctx.sampleRate * seconds);
  const buf = ctx.createBuffer(2, len, ctx.sampleRate);
  for (let ch = 0; ch < 2; ch++) {
    const d = buf.getChannelData(ch);
    for (let i = 0; i < len; i++) {
      const k = i / len;
      // pré-délai court + queue diffuse
      d[i] = i < ctx.sampleRate * 0.012 ? 0 : (Math.random() * 2 - 1) * Math.pow(1 - k, decay);
    }
  }
  return buf;
}

/** Enveloppe attaque / maintien / relâchement sur un gain. */
function env(g: AudioParam, t: number, peak: number, attack: number, hold: number, release: number): number {
  g.setValueAtTime(0.0001, t);
  g.linearRampToValueAtTime(peak, t + attack);
  g.setValueAtTime(peak, t + attack + hold);
  g.exponentialRampToValueAtTime(0.0001, t + attack + hold + release);
  return t + attack + hold + release + 0.05;
}

function osc(bus: MusicBus, type: OscillatorType, freq: number, t: number, end: number, detune = 0): OscillatorNode {
  const o = bus.ctx.createOscillator();
  o.type = type;
  o.frequency.setValueAtTime(freq, t);
  o.detune.value = detune;
  o.start(t);
  o.stop(end);
  return o;
}

/** Nappe chaude : 3 dents de scie désaccordées par note, filtre qui respire. */
export function pad(bus: MusicBus, notes: number[], t: number, dur: number, vol = 0.05, bright = 1): void {
  const out = bus.voice(0.7, 0.05);
  const lp = bus.ctx.createBiquadFilter();
  lp.type = 'lowpass';
  lp.Q.value = 0.7;
  lp.frequency.setValueAtTime(500 * bright, t);
  lp.frequency.linearRampToValueAtTime(1500 * bright, t + dur * 0.5);
  lp.frequency.linearRampToValueAtTime(700 * bright, t + dur);
  lp.connect(out);
  const end = env(out.gain, t, vol, Math.min(1.4, dur * 0.35), Math.max(0, dur - 1.6), 1.6);
  for (const n of notes) for (const det of [-9, 0, 8]) osc(bus, 'sawtooth', midi(n), t, end, det).connect(lp);
}

/** Chœur « aah » : dents de scie à travers deux formants. */
export function choir(bus: MusicBus, notes: number[], t: number, dur: number, vol = 0.04): void {
  const out = bus.voice(0.9, 0.1);
  const end = env(out.gain, t, vol, 0.9, Math.max(0, dur - 1.6), 1.4);
  for (const [f, q, g] of [
    [730, 6, 1],
    [1150, 8, 0.6],
  ] as const) {
    const bp = bus.ctx.createBiquadFilter();
    bp.type = 'bandpass';
    bp.frequency.value = f;
    bp.Q.value = q;
    const gg = bus.ctx.createGain();
    gg.gain.value = g;
    bp.connect(gg);
    gg.connect(out);
    for (const n of notes)
      for (const det of [-6, 6]) {
        const o = osc(bus, 'sawtooth', midi(n), t, end, det);
        const vib = bus.ctx.createOscillator();
        vib.frequency.value = 4.8;
        const vg = bus.ctx.createGain();
        vg.gain.value = 4;
        vib.connect(vg);
        vg.connect(o.detune);
        vib.start(t);
        vib.stop(end);
        o.connect(bp);
      }
  }
}

/** Cordes (ostinato) : dent de scie + vibrato, attaque nette. */
export function strings(bus: MusicBus, note: number, t: number, dur: number, vol = 0.035): void {
  const out = bus.voice(0.45, 0);
  const lp = bus.ctx.createBiquadFilter();
  lp.type = 'lowpass';
  lp.frequency.value = 2400;
  lp.connect(out);
  const end = env(out.gain, t, vol, 0.02, Math.max(0, dur - 0.08), 0.12);
  for (const det of [-5, 5]) osc(bus, 'sawtooth', midi(note), t, end, det).connect(lp);
}

/** Cuivres : accord avec ouverture de filtre (« stab » héroïque). */
export function brass(bus: MusicBus, notes: number[], t: number, dur: number, vol = 0.045): void {
  const out = bus.voice(0.5, 0.08);
  const lp = bus.ctx.createBiquadFilter();
  lp.type = 'lowpass';
  lp.Q.value = 2;
  lp.frequency.setValueAtTime(350, t);
  lp.frequency.exponentialRampToValueAtTime(2800, t + 0.08);
  lp.frequency.exponentialRampToValueAtTime(1100, t + Math.min(dur, 0.6));
  lp.connect(out);
  const end = env(out.gain, t, vol, 0.035, Math.max(0, dur - 0.2), 0.35);
  for (const n of notes) for (const det of [-7, 7]) osc(bus, 'sawtooth', midi(n), t, end, det).connect(lp);
}

/** Basse : sinus + octave triangle, filtrée. */
export function bass(bus: MusicBus, note: number, t: number, dur: number, vol = 0.12): void {
  const out = bus.voice(0.08, 0);
  const lp = bus.ctx.createBiquadFilter();
  lp.type = 'lowpass';
  lp.frequency.value = 420;
  lp.connect(out);
  const end = env(out.gain, t, vol, 0.03, Math.max(0, dur - 0.25), 0.25);
  osc(bus, 'sine', midi(note), t, end).connect(lp);
  const o2 = osc(bus, 'triangle', midi(note + 12), t, end);
  const g2 = bus.ctx.createGain();
  g2.gain.value = 0.35;
  o2.connect(g2);
  g2.connect(lp);
}

/** Cloche / célesta : partiels inharmoniques à décroissance longue. */
export function bell(bus: MusicBus, note: number, t: number, vol = 0.05, decay = 2.4): void {
  const out = bus.voice(0.6, 0.35);
  const f = midi(note);
  for (const [ratio, g, d] of [
    [1, 1, decay],
    [2.76, 0.35, decay * 0.5],
    [5.4, 0.15, decay * 0.25],
    [2, 0.25, decay * 0.7],
  ] as const) {
    const gn = bus.ctx.createGain();
    gn.connect(out);
    const end = env(gn.gain, t, vol * g, 0.004, 0, d);
    osc(bus, 'sine', f * ratio, t, end).connect(gn);
  }
  out.gain.value = 1;
}

/** Flûte : sinus + souffle, vibrato qui s'installe. */
export function flute(bus: MusicBus, note: number, t: number, dur: number, vol = 0.05): void {
  const out = bus.voice(0.6, 0.25);
  const end = env(out.gain, t, vol, 0.08, Math.max(0, dur - 0.2), 0.3);
  const o = osc(bus, 'sine', midi(note), t, end);
  const o2 = osc(bus, 'triangle', midi(note), t, end);
  const vib = bus.ctx.createOscillator();
  vib.frequency.value = 5.2;
  const vg = bus.ctx.createGain();
  vg.gain.setValueAtTime(0, t);
  vg.gain.linearRampToValueAtTime(12, t + Math.min(0.6, dur));
  vib.connect(vg);
  vg.connect(o.detune);
  vg.connect(o2.detune);
  vib.start(t);
  vib.stop(end);
  const g2 = bus.ctx.createGain();
  g2.gain.value = 0.4;
  o.connect(out);
  o2.connect(g2);
  g2.connect(out);
  const breath = bus.noise();
  const bp = bus.ctx.createBiquadFilter();
  bp.type = 'bandpass';
  bp.frequency.value = midi(note) * 2;
  bp.Q.value = 3;
  const bg = bus.ctx.createGain();
  bg.gain.value = 0.08;
  breath.connect(bp);
  bp.connect(bg);
  bg.connect(out);
  breath.start(t);
  breath.stop(end);
}

/** Pincé (arpèges) : triangle à décroissance rapide, filtre qui se referme. */
export function pluck(bus: MusicBus, note: number, t: number, vol = 0.04): void {
  const out = bus.voice(0.35, 0.3);
  const lp = bus.ctx.createBiquadFilter();
  lp.type = 'lowpass';
  lp.frequency.setValueAtTime(4200, t);
  lp.frequency.exponentialRampToValueAtTime(600, t + 0.35);
  lp.connect(out);
  const end = env(out.gain, t, vol, 0.004, 0, 0.55);
  osc(bus, 'triangle', midi(note), t, end).connect(lp);
  osc(bus, 'square', midi(note), t, end, 3).connect(lp);
}

/** Taiko : coup grave à chute de hauteur + attaque de bruit. */
export function taiko(bus: MusicBus, t: number, vol = 0.32, pitch = 1): void {
  const out = bus.voice(0.25, 0);
  const end = env(out.gain, t, vol, 0.003, 0, 0.55);
  const o = bus.ctx.createOscillator();
  o.type = 'sine';
  o.frequency.setValueAtTime(110 * pitch, t);
  o.frequency.exponentialRampToValueAtTime(42 * pitch, t + 0.35);
  o.connect(out);
  o.start(t);
  o.stop(end);
  const n = bus.noise();
  const lp = bus.ctx.createBiquadFilter();
  lp.type = 'lowpass';
  lp.frequency.value = 900;
  const ng = bus.ctx.createGain();
  env(ng.gain, t, vol * 0.5, 0.002, 0, 0.09);
  n.connect(lp);
  lp.connect(ng);
  ng.connect(out);
  n.start(t);
  n.stop(t + 0.2);
}

/** Caisse claire militaire : bruit filtré + corps. */
export function snare(bus: MusicBus, t: number, vol = 0.12): void {
  const out = bus.voice(0.3, 0);
  const n = bus.noise();
  const bp = bus.ctx.createBiquadFilter();
  bp.type = 'bandpass';
  bp.frequency.value = 2200;
  bp.Q.value = 0.8;
  n.connect(bp);
  bp.connect(out);
  const end = env(out.gain, t, vol, 0.002, 0, 0.16);
  n.start(t);
  n.stop(end);
  const g = bus.ctx.createGain();
  env(g.gain, t, vol * 0.6, 0.002, 0, 0.08);
  osc(bus, 'triangle', 190, t, t + 0.12).connect(g);
  g.connect(out);
}

/** Montée de cymbale (souffle aigu crescendo). */
export function swell(bus: MusicBus, t: number, dur: number, vol = 0.06): void {
  const out = bus.voice(0.6, 0);
  const n = bus.noise();
  const hp = bus.ctx.createBiquadFilter();
  hp.type = 'highpass';
  hp.frequency.setValueAtTime(3000, t);
  hp.frequency.linearRampToValueAtTime(8000, t + dur);
  n.connect(hp);
  hp.connect(out);
  out.gain.setValueAtTime(0.0001, t);
  out.gain.exponentialRampToValueAtTime(vol, t + dur);
  out.gain.exponentialRampToValueAtTime(0.0001, t + dur + 0.6);
  n.start(t);
  n.stop(t + dur + 0.7);
}

/** Crash : impact de cymbale. */
export function crash(bus: MusicBus, t: number, vol = 0.06): void {
  const out = bus.voice(0.5, 0);
  const n = bus.noise();
  const hp = bus.ctx.createBiquadFilter();
  hp.type = 'highpass';
  hp.frequency.value = 5000;
  n.connect(hp);
  hp.connect(out);
  const end = env(out.gain, t, vol, 0.003, 0, 1.6);
  n.start(t);
  n.stop(end);
}

/** Nappe douce (thème de partie) : sinus + triangle légèrement désaccordés, filtre bas, attaque lente. */
export function softPad(bus: MusicBus, notes: number[], t: number, dur: number, vol = 0.03, bright = 1): void {
  const out = bus.voice(0.8, 0);
  const lp = bus.ctx.createBiquadFilter();
  lp.type = 'lowpass';
  lp.Q.value = 0.5;
  lp.frequency.setValueAtTime(600 * bright, t);
  lp.frequency.linearRampToValueAtTime(1100 * bright, t + dur * 0.5);
  lp.frequency.linearRampToValueAtTime(650 * bright, t + dur);
  lp.connect(out);
  const end = env(out.gain, t, vol, Math.min(2.6, dur * 0.4), Math.max(0, dur - 2.6), 3);
  for (const n of notes) {
    osc(bus, 'triangle', midi(n), t, end, -5).connect(lp);
    osc(bus, 'sine', midi(n), t, end, 6).connect(lp);
  }
}

/** Kalimba / boîte à musique : sinus et partiel doux, décroissance moyenne, écho léger. */
export function kalimba(bus: MusicBus, note: number, t: number, vol = 0.03): void {
  const out = bus.voice(0.55, 0.22);
  const f = midi(note);
  for (const [ratio, g, d] of [
    [1, 1, 1.4],
    [3, 0.12, 0.35],
  ] as const) {
    const gn = bus.ctx.createGain();
    gn.connect(out);
    const end = env(gn.gain, t, vol * g, 0.006, 0, d);
    osc(bus, 'sine', f * ratio, t, end).connect(gn);
  }
  out.gain.value = 1;
}
