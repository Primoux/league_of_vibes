import { describe, expect, it } from 'vitest';
import { parseClientMsg } from '../src/net/validate.ts';

describe('validation useItem', () => {
  it('accepte les slots −1..5 avec ou sans targetPos', () => {
    for (const slot of [-1, 0, 3, 5]) expect(parseClientMsg(JSON.stringify({ t: 'useItem', slot }))).not.toBeNull();
    expect(parseClientMsg(JSON.stringify({ t: 'useItem', slot: 2, targetPos: { x: 100, y: 200 } }))).not.toBeNull();
  });
  it('rejette les slots hors bornes et les positions invalides', () => {
    for (const slot of [-2, 6, 1.5, '1', null]) expect(parseClientMsg(JSON.stringify({ t: 'useItem', slot }))).toBeNull();
    expect(parseClientMsg(JSON.stringify({ t: 'useItem', slot: 1, targetPos: { x: 'a', y: 0 } }))).toBeNull();
  });
});
