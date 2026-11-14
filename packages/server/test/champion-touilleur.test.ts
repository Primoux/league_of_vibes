import { describe, expect, it } from 'vitest';
import { WALLS, getChampion } from '@lov/shared';
import { addBuff, canAttack, canCast, canMove, cleanse, getBuff, hasCC } from '../src/combat/buffs.ts';
import { applyDamage } from '../src/combat/damage.ts';
import { isWalkable } from '../src/abilities/primitives.ts';
import { advance, cast, damage, dummy, game } from './champion-helpers.ts';

function ultimate(facing = 0) {
  const state = game('touilleur'); state.target.facing = facing;
  expect(cast(state.world, state.source, 'R', state.target)).toBe(true);
  advance(state.world, .26);
  return state;
}

describe('Le Touilleur', () => {
  it('possède un kit jouable et un ultime de deux secondes de base', () => {
    expect(getChampion('touilleur').name).toBe('Le Touilleur');
    expect(getChampion('touilleur').abilities.R.params!.duration).toBe(2);
  });
  it('Q touche et ralentit uniquement le premier ennemi', () => {
    const { world, source, target } = game('touilleur'), behind = dummy(world, source, { x: 1500, y: 1000 });
    cast(world, source, 'Q', target); advance(world, .5);
    expect(damage(world, 'touilleur_q', target)).toBeGreaterThan(0); expect(hasCC(target, 'slow')).toBe(true);
    expect(damage(world, 'touilleur_q', behind)).toBe(0);
  });
  it('W respecte sa télégraphie et épargne les alliés et les cibles hors zone', () => {
    const { world, source, target } = game('touilleur'), ally = dummy(world, source, target.pos), far = dummy(world, source, { x: 1800, y: 1000 });
    ally.team = source.team;
    cast(world, source, 'W', target); advance(world, .5);
    expect(damage(world, 'touilleur_w')).toBe(0); advance(world, .15);
    expect(damage(world, 'touilleur_w', target)).toBeGreaterThan(0); expect(hasCC(target, 'slow')).toBe(true);
    expect(damage(world, 'touilleur_w', ally)).toBe(0); expect(damage(world, 'touilleur_w', far)).toBe(0);
  });
  it('E borne son déplacement et son bouclier expire', () => {
    const { world, source } = game('touilleur');
    cast(world, source, 'E', undefined, { x: 1700, y: 1000 });
    expect(source.pos.x).toBeCloseTo(1325); expect(source.shield).toBe(45);
    advance(world, 2.1); expect(source.shield).toBe(0);
  });
  it.each(['root', 'grounded'] as const)('%s empêche E et R sans dépenser les ressources', control => {
    const { world, source, target } = game('touilleur'), mana = source.mana;
    addBuff(world, source, control, { duration: 1 });
    expect(cast(world, source, 'E')).toBe(false); expect(cast(world, source, 'R', target)).toBe(false);
    expect(source.mana).toBe(mana); expect(source.champ!.cooldowns.R).toBe(0);
  });
  it.each([0, Math.PI / 2, Math.PI, -Math.PI / 2])('R arrive derrière une cible orientée à %s radians', facing => {
    const { world, source, target } = ultimate(facing);
    const dx = source.pos.x - target.pos.x, dy = source.pos.y - target.pos.y;
    expect(dx * Math.cos(facing) + dy * Math.sin(facing)).toBeLessThan(-target.radius);
    expect(Math.hypot(dx, dy)).toBeGreaterThanOrEqual(source.radius + target.radius);
    expect(isWalkable(source.pos, source.radius)).toBe(true);
    expect(damage(world, 'touilleur_r', target)).toBeGreaterThan(0);
  });
  it('R peut rejoindre le dos même à la limite de portée avec les rayons des unités', () => {
    const { world, source, target } = game('touilleur'); target.pos.x = source.pos.x + 450 + source.radius + target.radius; target.facing = Math.PI;
    expect(cast(world, source, 'R', target)).toBe(true); advance(world, .26);
    expect(source.pos.x).toBeGreaterThan(target.pos.x); expect(hasCC(target, 'stun')).toBe(true);
  });
  it('R étourdit deux secondes et bloque le lanceur pendant la canalisation', () => {
    const { world, source, target } = ultimate();
    expect(getBuff(target, 'stun', source.id)?.duration).toBe(2);
    expect(canMove(target)).toBe(false); expect(canAttack(target)).toBe(false); expect(canCast(target)).toBe(false);
    expect(canMove(source)).toBe(false); expect(canAttack(source)).toBe(false); expect(canCast(source)).toBe(false);
    advance(world, 1.9); expect(hasCC(target, 'stun')).toBe(true); expect(source.data.channeling).toBe(true);
    advance(world, .11); expect(hasCC(target, 'stun')).toBe(false); expect(source.data.channeling).toBeUndefined(); expect(canMove(source)).toBe(true);
  });
  it('la ténacité réduit le stun et la canalisation ensemble', () => {
    const { world, source, target } = game('touilleur'); addBuff(world, target, 'tenacious', { duration: 10, params: { tenacity: .5 } });
    cast(world, source, 'R', target); advance(world, .26);
    expect(getBuff(target, 'stun', source.id)?.duration).toBe(1);
    advance(world, 1.01); expect(hasCC(target, 'stun')).toBe(false); expect(source.data.channeling).toBeUndefined();
  });
  it('la purification termine aussitôt le stun et le moulinet sans le réappliquer', () => {
    const { world, source, target } = ultimate(); cleanse(world, target);
    expect(hasCC(target, 'stun')).toBe(false); expect(source.data.channeling).toBeUndefined();
    advance(world, 3); expect(hasCC(target, 'stun')).toBe(false); expect(getBuff(source, 'touilleur_r_channel')).toBeUndefined();
  });
  it.each(['stun', 'silence', 'knockup'] as const)('%s sur le lanceur interrompt R sans retirer les contrôles d’une autre source', control => {
    const { world, source, target } = ultimate(), other = dummy(world, source);
    addBuff(world, target, 'stun', { source: other, duration: 3 });
    addBuff(world, source, control, { duration: 1 }); advance(world, .04);
    expect(source.data.channeling).toBeUndefined(); expect(getBuff(target, 'stun', source.id)).toBeUndefined();
    expect(getBuff(target, 'stun', other.id)).toBeDefined();
  });
  it.each(['source', 'target'] as const)('la mort de %s arrête R sans verrou résiduel', side => {
    const state = ultimate(), victim = state[side], killer = side === 'source' ? state.target : state.source;
    applyDamage(state.world, { source: killer, target: victim, amount: 1e6, type: 'true' });
    expect(state.source.data.channeling).toBeUndefined(); expect(getBuff(state.target, 'stun', state.source.id)).toBeUndefined();
    advance(state.world, 3); expect(state.source.data.channeling).toBeUndefined();
  });
  it('un déplacement forcé de cible rompt la touille', () => {
    const { world, source, target } = ultimate(); target.pos.x += 500; advance(world, .04);
    expect(source.data.channeling).toBeUndefined(); expect(hasCC(target, 'stun')).toBe(false);
  });
  it.each(['source', 'target'] as const)('la stase de %s rompt la touille', side => {
    const state = ultimate(); state[side].data.stasis = true; advance(state.world, .04);
    expect(state.source.data.channeling).toBeUndefined(); expect(getBuff(state.target, 'stun', state.source.id)).toBeUndefined();
  });
  it('une cible supprimée libère le lanceur au tick suivant', () => {
    const { world, source, target } = ultimate(); world.remove(target.id); advance(world, .04);
    expect(source.data.channeling).toBeUndefined(); expect(canMove(source)).toBe(true);
  });
  it('R n’arrive ni dans un mur ni devant sa cible si son dos est bloqué', () => {
    const { world, source, target } = game('touilleur'), wall = WALLS[0]!;
    const edge = Math.min(...wall.map(p => p.x)), y = wall.reduce((sum, p) => sum + p.y, 0) / wall.length;
    target.pos = { x: edge - target.radius - 2, y }; target.facing = Math.PI;
    source.pos = { x: target.pos.x - 250, y };
    expect(isWalkable(source.pos, source.radius)).toBe(true); expect(isWalkable(target.pos, target.radius)).toBe(true);
    cast(world, source, 'R', target); advance(world, .26);
    expect(isWalkable(source.pos, source.radius)).toBe(true);
    if (hasCC(target, 'stun')) expect((source.pos.x - target.pos.x) * Math.cos(target.facing)).toBeLessThan(-target.radius * .5);
    else { expect(damage(world, 'touilleur_r')).toBe(0); expect(source.data.channeling).toBeUndefined(); }
  });
  it('un dos collé à la limite de carte ne permet pas d’étourdir depuis l’avant', () => {
    const { world, source, target } = game('touilleur'); target.pos = { x: target.radius, y: 1000 }; target.facing = 0; source.pos = { x: 350, y: 1000 };
    cast(world, source, 'R', target); advance(world, .26);
    expect(source.pos.x).toBe(350); expect(hasCC(target, 'stun')).toBe(false); expect(damage(world, 'touilleur_r')).toBe(0);
  });
  it('un ordre stop du lanceur annule le canal', () => {
    const { world, source, target } = ultimate(); world.commands.push({ playerId: source.champ!.playerId, msg: { t: 'stop' } }); advance(world, .04);
    expect(source.data.channeling).toBeUndefined(); expect(hasCC(target, 'stun')).toBe(false);
  });
  it('R refuse les sbires, alliés et cibles invisibles sans coût', () => {
    const { world, source, target } = game('touilleur'), minion = dummy(world, source, target.pos), mana = source.mana;
    expect(cast(world, source, 'R', minion)).toBe(false);
    target.team = source.team; expect(cast(world, source, 'R', target)).toBe(false);
    target.team = 'red'; addBuff(world, target, 'invisible', { duration: 3 }); expect(cast(world, source, 'R', target)).toBe(false);
    expect(source.mana).toBe(mana); expect(source.champ!.cooldowns.R).toBe(0);
  });
  it('R interrompu avant son départ ne téléporte pas et ne stun pas', () => {
    const { world, source, target } = game('touilleur'); cast(world, source, 'R', target);
    addBuff(world, source, 'stun', { duration: 1 }); advance(world, .4);
    expect(source.pos).toEqual({ x: 1000, y: 1000 }); expect(hasCC(target, 'stun')).toBe(false); expect(source.data.channeling).toBeUndefined();
  });
  it('le passif attend trois impacts espacés et ne proc ni sur AA ni sur sbires', () => {
    const { world, source, target } = game('touilleur');
    for (let i = 0; i < 8; i++) applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'touilleur_q' });
    expect(getBuff(source, 'touilleur_count')?.stacks).toBe(1);
    advance(world, .41); applyDamage(world, { source, target, amount: 1, type: 'true', isAutoAttack: true });
    applyDamage(world, { source, target: dummy(world, source), amount: 1, type: 'true', ability: 'touilleur_q' });
    expect(getBuff(source, 'touilleur_count')?.stacks).toBe(1);
    for (let i = 0; i < 2; i++) { applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'touilleur_q' }); advance(world, .41); }
    expect(damage(world, 'touilleur_p', target)).toBeGreaterThan(0); expect(getBuff(source, 'touilleur_count')).toBeUndefined(); expect(getBuff(source, 'touilleur_p_speed')).toBeDefined();
  });
  it('un proc passif déjà létal ne cause pas de seconde mort', () => {
    const { world, source, target } = game('touilleur');
    for (let i = 0; i < 2; i++) { applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'touilleur_q' }); advance(world, .41); }
    target.hp = 1; applyDamage(world, { source, target, amount: 2, type: 'true', ability: 'touilleur_q' });
    expect(target.dead).toBe(true); expect(damage(world, 'touilleur_p')).toBe(0);
    expect(world.events.pending.filter(e => e.type === 'death' && e.target === target.id)).toHaveLength(1);
  });
});
