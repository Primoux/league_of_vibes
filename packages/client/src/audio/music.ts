/**
 * Musique d'ambiance générée (pas d'asset) : pads d'accords en La mineur + mélodie de pluck
 * pentatonique procédurale, et jingles de victoire/défaite. Propriétaire : mistral-economy.
 */
import { audio } from './AudioManager.ts';
import { tone } from './synth.ts';

// Progression en La mineur (fréquences des accords, la = 220 Hz)
const CHORDS: number[][] = [
  [220, 261.6, 329.6], // La m
  [174.6, 220, 261.6], // Fa
  [261.6, 329.6, 392], // Do
  [196, 246.9, 293.7], // Sol
];
const PENTA = [440, 523.3, 587.3, 659.3, 784, 880]; // La mineur pentatonique

class Music {
  private timer: ReturnType<typeof setInterval> | null = null;
  private chordIndex = 0;
  private playing = false;

  /** Démarre la boucle d'ambiance (appelé au début de la partie). */
  start(): void {
    if (this.playing) return;
    this.playing = true;
    this.chordIndex = 0;
    this.scheduleChord();
    this.timer = setInterval(() => this.scheduleChord(), 3800);
  }

  stop(): void {
    this.playing = false;
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }

  private scheduleChord(): void {
    if (!this.playing || !audio.ready) return;
    const chord = CHORDS[this.chordIndex % CHORDS.length]!;
    this.chordIndex++;
    audio.sfx((ctx, dest) => {
      // pad : deux oscillateurs légèrement désaccordés par note
      for (const f of chord) {
        tone(ctx, dest, { type: 'triangle', freq: f, dur: 3.6, vol: 0.05, attack: 0.8, filter: 1200 });
        tone(ctx, dest, { type: 'triangle', freq: f * 1.005, dur: 3.6, vol: 0.04, attack: 0.8, filter: 1200 });
      }
      // basse
      tone(ctx, dest, { type: 'sine', freq: chord[0]! / 2, dur: 3.4, vol: 0.08, attack: 0.4 });
      // pluck mélodique clairsemé (1 note sur 2 accords)
      if (this.chordIndex % 2 === 0) {
        const f = PENTA[Math.floor(Math.random() * PENTA.length)]!;
        tone(ctx, dest, { type: 'sine', freq: f, dur: 0.5, vol: 0.06, attack: 0.01, delay: 0.4 + Math.random() * 1.5 });
      }
    }, { channel: 'music' });
  }

  /** Jingle de fin de partie. */
  stinger(kind: 'victory' | 'defeat'): void {
    if (!audio.ready) return;
    const seq = kind === 'victory' ? [523, 659, 784, 1047] : [392, 330, 262, 196];
    audio.sfx((ctx, dest) => {
      seq.forEach((f, i) => {
        tone(ctx, dest, { type: 'triangle', freq: f, dur: 0.35, vol: 0.22, delay: i * 0.22, filter: 2500 });
        tone(ctx, dest, { type: 'sine', freq: f / 2, dur: 0.35, vol: 0.18, delay: i * 0.22 });
      });
    }, { channel: 'music' });
  }
}

export const music = new Music();
