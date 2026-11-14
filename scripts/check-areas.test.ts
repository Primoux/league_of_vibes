import { describe, expect, it } from 'vitest';
import { execFileSync } from 'node:child_process';

/** Lance le garde-fou de périmètre sur une liste de fichiers simulée. */
function run(args: string[]): { ok: boolean; out: string } {
  try {
    const out = execFileSync('node', ['scripts/check-areas.mjs', ...args], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    return { ok: true, out };
  } catch (e) {
    const err = e as { stdout?: string; stderr?: string };
    return { ok: false, out: `${err.stdout ?? ''}${err.stderr ?? ''}` };
  }
}

/** Les labels que la CI posera sur la PR, un par ligne. */
const labels = (title: string, body = ''): string[] =>
  run(['--labels', '--title', title, '--body', body, '--files', 'packages/client/src/ui/hud/Hud.tsx'])
    .out.split('\n')
    .filter(Boolean);

function check(title: string, files: string[], body = ''): { ok: boolean; out: string } {
  try {
    const out = execFileSync('node', ['scripts/check-areas.mjs', '--title', title, '--files', files.join(','), '--body', body], {
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

describe('check-areas : fonctionnalité qui traverse plusieurs areas', () => {
  const boutique = [
    'packages/client/src/ui/hud/Shop.tsx',
    'packages/client/src/ui/logic/shop.ts',
    'packages/client/src/ui/redesign.css',
  ];

  it('refuse deux areas quand une seule est déclarée', () => {
    const r = check('feat(ui-hud): boutique plus claire', boutique);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('logic/shop.ts');
  });

  it('accepte un scope à plusieurs areas dans le titre', () => {
    const r = check('feat(ui-hud,ui-core): boutique plus claire', boutique);
    expect(r.ok).toBe(true);
    expect(r.out).toContain('ui-hud, ui-core');
  });

  it('accepte des areas déclarées dans le corps de la PR', () => {
    const r = check('feat(ui-hud): boutique plus claire', boutique, 'Résumé\n\nAreas: ui-hud, ui-core\n');
    expect(r.ok).toBe(true);
  });

  it('refuse une area inconnue parmi plusieurs', () => {
    const r = check('feat(ui-hud,cuisine): boutique', boutique);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('cuisine');
  });

  it('avertit sans bloquer au-delà de trois areas', () => {
    const r = check('feat(input,ui-hud,ui-core,client-net,render): mode jeu', [
      'packages/client/src/input/gameMode.ts',
      'packages/client/src/ui/hud/Hud.tsx',
      'packages/client/src/ui/styles.css',
      'packages/client/src/state.ts',
      'packages/client/src/main.ts',
    ]);
    expect(r.ok).toBe(true);
    expect(r.out).toContain('5 areas déclarées');
  });

  it('refuse toujours un contrat, même avec plusieurs areas', () => {
    const r = check('feat(ui-hud,ui-core): boutique', [...boutique, 'packages/shared/src/schema.ts']);
    expect(r.ok).toBe(false);
  });
});

describe('check-areas : labels posés sur la PR', () => {
  it('une area → un label', () => {
    expect(labels('feat(ui-hud): barre de mana')).toEqual(['area:ui-hud']);
  });

  it('plusieurs areas dans le titre → un label chacune', () => {
    expect(labels('feat(ui-hud,ui-core): boutique')).toEqual(['area:ui-hud', 'area:ui-core']);
  });

  it('areas déclarées dans le corps → prises en compte', () => {
    expect(labels('feat(input): mode jeu', 'Areas: input, ui-hud, client-net')).toEqual([
      'area:input',
      'area:ui-hud',
      'area:client-net',
    ]);
  });

  it('PR de contrat → label « contrat »', () => {
    expect(labels('contract: ajoute visionScore')).toEqual(['contrat']);
  });

  it('area inconnue → aucun label inventé', () => {
    expect(labels('feat(cuisine): soupe')).toEqual([]);
  });
});

describe('check-areas : couverture du dépôt', () => {
  it('aucun fichier suivi n’échappe à toutes les areas', () => {
    // Un fichier que personne ne couvre est intouchable : la CI refuse toute PR qui le modifie,
    // quelle que soit l'area annoncée. Ce test a attrapé 13 fichiers — dont toutes les données
    // de jeu partagées (structures, sbires, monstres, objets), ce qui rendait tout travail
    // d'équilibrage impossible.
    const r = run(['--audit']);
    expect(r.out).toBe('✓ Tous les fichiers suivis sont couverts par une area.\n');
    expect(r.ok).toBe(true);
  });

  it('les données de jeu partagées appartiennent à l’area qui possède leur comportement', () => {
    const cases: [string, string][] = [
      ['packages/shared/src/data/structures.ts', 'feat(systems): plaques de tour'],
      ['packages/shared/src/data/minions.ts', 'fix(systems): dégâts des sbires'],
      ['packages/shared/src/data/monsters.ts', 'fix(world-ai): camps de jungle'],
      ['packages/shared/src/data/items.ts', 'feat(items): nouvel objet'],
      ['packages/shared/src/data/buffs.ts', 'feat(abilities): nouveau CC'],
      ['packages/shared/src/data/summoners.ts', 'fix(abilities): sort d’invocateur'],
    ];
    for (const [file, title] of cases) expect(check(title, [file]).ok, `${title} → ${file}`).toBe(true);
  });
});
