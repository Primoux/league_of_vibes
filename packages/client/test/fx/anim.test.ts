import { describe, expect, it } from 'vitest';
import { DEFAULT_CTX, GENERIC_CLIPS } from '../../src/render/champions/anim/clips.ts';
import { Animator, BODY_OFFSET, J, POSE_SIZE, blendPose, createPose, resolveClip } from '../../src/render/champions/anim/pose.ts';

describe('poses', () => {
  it('blendPose interpole chaque composante', () => {
    const a = createPose();
    const b = createPose().fill(2);
    const out = createPose();
    blendPose(a, b, 0.25, out);
    expect(out.every((v) => v === 0.5)).toBe(true);
    expect(out.length).toBe(POSE_SIZE);
  });

  it('resolveClip suit la chaîne de repli', () => {
    expect(resolveClip(GENERIC_CLIPS, 'cast_R')).toBe('cast_R');
    expect(resolveClip(GENERIC_CLIPS, 'cast_Q')).toBe('cast');
    expect(resolveClip(GENERIC_CLIPS, 'inconnu')).toBe('idle');
  });

  it('tous les clips génériques produisent des valeurs finies', () => {
    const p = createPose();
    for (const [name, clip] of Object.entries(GENERIC_CLIPS)) {
      for (let t = 0; t <= clip.duration * 2; t += 0.05) {
        p.fill(0);
        clip.sample(t, p, DEFAULT_CTX);
        expect(p.every(Number.isFinite), name).toBe(true);
      }
    }
  });
});

describe('Animator', () => {
  it('revient au clip de base après un one-shot', () => {
    const a = new Animator(GENERIC_CLIPS, DEFAULT_CTX);
    a.setBase('run');
    a.update(0.5);
    expect(a.current).toBe('run');
    a.play('cast_Q');
    expect(a.current).toBe('cast');
    a.update(0.3);
    expect(a.current).toBe('cast');
    a.update(0.3); // > 0.5 s
    expect(a.current).toBe('run');
  });

  it('changer la base pendant un one-shot ne l’interrompt pas', () => {
    const a = new Animator(GENERIC_CLIPS, DEFAULT_CTX);
    a.play('strike');
    a.setBase('attackReady');
    expect(a.current).toBe('strike');
    a.update(0.5);
    expect(a.current).toBe('attackReady');
  });

  it('la mort est un état final', () => {
    const a = new Animator(GENERIC_CLIPS, DEFAULT_CTX);
    a.play('death');
    a.update(3);
    expect(a.current).toBe('death');
    expect(a.pose[J.body * 3 + 2]).toBeGreaterThan(1.2); // couché (≈ 80°)
    expect(a.pose[BODY_OFFSET + 1]).toBeGreaterThanOrEqual(0); // pivot aux pieds : le corps couché ne s'enfonce pas
  });

  it('fondu : la pose part de la précédente', () => {
    const a = new Animator(GENERIC_CLIPS, DEFAULT_CTX);
    a.play('death');
    a.update(3);
    const fallen = a.pose[J.body * 3 + 2]!;
    a.setBase('idle');
    a.play('idle', { fade: 0.2 });
    a.update(0.01);
    expect(a.pose[J.body * 3 + 2]).toBeGreaterThan(fallen * 0.9); // encore presque couché
    a.update(0.5);
    expect(Math.abs(a.pose[J.body * 3 + 2]!)).toBeLessThan(0.01);
  });

  it('rate accélère les boucles', () => {
    const a = new Animator(GENERIC_CLIPS, DEFAULT_CTX);
    const b = new Animator(GENERIC_CLIPS, DEFAULT_CTX);
    a.setBase('run');
    b.setBase('run');
    a.update(1);
    b.update(1);
    b.rate = 2;
    a.update(0.1);
    b.update(0.05);
    expect(Array.from(b.pose)).toEqual(Array.from(a.pose).map((v) => expect.closeTo(v, 5)));
  });
});
