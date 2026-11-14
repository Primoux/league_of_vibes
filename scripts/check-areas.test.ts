import { describe, expect, it } from 'vitest';
import { execFileSync } from 'node:child_process';

/** Lance le garde-fou de périmètre sur une liste de fichiers simulée. */
function check(title: string, files: string[]): { ok: boolean; out: string } {
  try {
    const out = execFileSync('node', ['scripts/check-areas.mjs', '--title', title, '--files', files.join(',')], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    return { ok: true, out };
  } catch (e) {
    const err = e as { stdout?: string; stderr?: string };
    return { ok: false, out: `${err.stdout ?? ''}${err.stderr ?? ''}` };
  }
}

describe('check-areas : une PR = une area', () => {
  it('accepte une PR dans son area', () => {
    expect(check('feat(ui-hud): barre de mana', ['packages/client/src/ui/hud/Hud.tsx']).ok).toBe(true);
  });

  it('refuse un fichier hors de son area', () => {
    const r = check('feat(ui-hud): barre de mana', [
      'packages/client/src/ui/hud/Hud.tsx',
      'packages/server/src/nav/grid.ts',
    ]);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('nav/grid.ts');
  });

  it('refuse un titre sans area', () => {
    expect(check('ajoute un truc', ['packages/client/src/ui/hud/Hud.tsx']).ok).toBe(false);
  });

  it('refuse une area inconnue', () => {
    expect(check('feat(cuisine): soupe', ['packages/client/src/ui/hud/Hud.tsx']).ok).toBe(false);
  });

  it('autorise docs et tests depuis n’importe quelle area', () => {
    expect(
      check('feat(nav): champ de flux', [
        'packages/server/src/nav/flow.ts',
        'packages/server/test/nav.test.ts',
        'docs/notes.md',
      ]).ok,
    ).toBe(true);
  });
});

describe('check-areas : contrats partagés', () => {
  it('refuse un contrat dans une PR normale', () => {
    const r = check('feat(nav): champ de flux', ['packages/server/src/nav/flow.ts', 'packages/shared/src/types.ts']);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('contract');
  });

  it('accepte une PR de contrat qui ne contient que des contrats', () => {
    expect(check('contract: ajoute visionScore', ['packages/shared/src/types.ts', 'packages/shared/src/protocol.ts']).ok).toBe(true);
  });

  it('refuse une PR de contrat qui embarque du code', () => {
    const r = check('contract: ajoute visionScore', ['packages/shared/src/types.ts', 'packages/server/src/nav/flow.ts']);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('nav/flow.ts');
  });
});

describe('check-areas : tranche verticale champion', () => {
  const nyx = [
    'packages/shared/src/data/champions/nyx.ts',
    'packages/server/src/abilities/champions/nyx.ts',
    'packages/client/src/render/champions/models/nyx.ts',
    'packages/client/src/fx/champions/nyx.ts',
    'packages/shared/src/data/champions/index.ts',
  ];

  it('accepte un champion de bout en bout', () => {
    const r = check('feat(champion): Nyx', nyx);
    expect(r.ok).toBe(true);
    expect(r.out).toContain('nyx');
  });

  it('refuse deux champions dans la même PR', () => {
    const r = check('feat(champion): Nyx et Zephyr', [...nyx, 'packages/client/src/fx/champions/zephyr.ts']);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('zephyr');
  });
});
