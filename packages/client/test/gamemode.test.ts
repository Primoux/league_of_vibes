import { describe, expect, it } from 'vitest';
import { keyLetter } from '../src/input/gameMode.ts';

const ev = (key: string, code: string) => ({ key, code }) as KeyboardEvent;

describe('keyLetter', () => {
  it('utilise e.key sans modificateur (AZERTY : touche physique KeyA = Q)', () => {
    expect(keyLetter(ev('q', 'KeyA'))).toBe('Q');
    expect(keyLetter(ev('Z', 'KeyW'))).toBe('Z');
  });

  it('retombe sur la position physique quand Alt/Option produit un caractère spécial', () => {
    expect(keyLetter(ev('œ', 'KeyQ'))).toBe('Q');
    expect(keyLetter(ev('∑', 'KeyW'))).toBe('W');
  });

  it('ignore les touches qui ne sont pas des lettres', () => {
    expect(keyLetter(ev('Alt', 'AltLeft'))).toBe('');
    expect(keyLetter(ev('1', 'Digit1'))).toBe('');
    expect(keyLetter(ev(' ', 'Space'))).toBe('');
  });
});
