import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

/** Les attentes de vitest dont ce fichier se sert, sur `node:assert` (lancé par `node --test`). */
function expect(actual: unknown, message?: string) {
  return {
    toBe: (v: unknown) => assert.equal(actual, v, message),
    toEqual: (v: unknown) => assert.deepEqual(actual, v, message),
    toContain: (v: string) => assert.ok(String(actual).includes(v), message ?? `« ${v} » attendu dans : ${String(actual)}`),
  };
}

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
  run(['--labels', '--title', title, '--body', body, '--files', 'crates/lov-client/src/ui/hud/mod.rs'])
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
    expect(check('feat(ui-hud): barre de mana', ['crates/lov-client/src/ui/hud/mod.rs']).ok).toBe(true);
  });

  it('refuse un fichier hors de son area', () => {
    const r = check('feat(ui-hud): barre de mana', [
      'crates/lov-client/src/ui/hud/mod.rs',
      'crates/lov-sim/src/nav/grid.rs',
    ]);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('nav/grid.rs');
  });

  it('refuse un titre sans area', () => {
    expect(check('ajoute un truc', ['crates/lov-client/src/ui/hud/mod.rs']).ok).toBe(false);
  });

  it('refuse une area inconnue', () => {
    expect(check('feat(cuisine): soupe', ['crates/lov-client/src/ui/hud/mod.rs']).ok).toBe(false);
  });

  it('autorise docs et tests depuis n’importe quelle area', () => {
    expect(
      check('feat(nav): champ de flux', [
        'crates/lov-sim/src/nav/flow.rs',
        'crates/lov-sim/tests/nav.rs',
        'docs/notes.md',
      ]).ok,
    ).toBe(true);
  });
});

describe('check-areas : contrats partagés', () => {
  it('refuse un contrat dans une PR normale', () => {
    const r = check('feat(nav): champ de flux', ['crates/lov-sim/src/nav/flow.rs', 'crates/lov-shared/src/types.rs']);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('contract');
  });

  it('accepte une PR de contrat qui ne contient que des contrats', () => {
    expect(check('contract: ajoute visionScore', ['crates/lov-shared/src/types.rs', 'crates/lov-shared/src/protocol.rs']).ok).toBe(true);
  });

  it('refuse une PR de contrat qui embarque du code', () => {
    const r = check('contract: ajoute visionScore', ['crates/lov-shared/src/types.rs', 'crates/lov-sim/src/nav/flow.rs']);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('nav/flow.rs');
  });
});

describe('check-areas : tranche verticale champion', () => {
  // `mod.rs` est le registre partagé de la tranche.
  const nyxRust = [
    'crates/lov-shared/src/data/champions/nyx.rs',
    'crates/lov-sim/src/abilities/champions/nyx.rs',
    'crates/lov-client/src/render/champions/models/nyx.rs',
    'crates/lov-client/src/fx/champions/nyx.rs',
    'crates/lov-shared/src/data/champions/mod.rs',
  ];

  it('accepte un champion de bout en bout, registre mod.rs compris', () => {
    const r = check('feat(champion): Nyx en Rust', nyxRust);
    expect(r.ok).toBe(true);
    expect(r.out).toContain('nyx');
  });

  it('refuse deux champions dans la même PR', () => {
    const r = check('feat(champion): Nyx et Zephyr', [...nyxRust, 'crates/lov-client/src/fx/champions/zephyr.rs']);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('zephyr');
  });
});

describe('check-areas : contrats Rust', () => {
  it('refuse le protocole Rust dans une PR normale', () => {
    const r = check('feat(net): salles', ['crates/lov-server/src/room.rs', 'crates/lov-shared/src/protocol.rs']);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('protocol.rs');
  });

  it('accepte une PR de contrat qui crée lov-shared et met à jour Cargo.lock', () => {
    const r = check('contract: protocole v2 en Rust', [
      'crates/lov-shared/Cargo.toml',
      'crates/lov-shared/src/lib.rs',
      'crates/lov-shared/src/protocol.rs',
      'Cargo.lock',
    ]);
    expect(r.ok).toBe(true);
  });
});

describe('check-areas : registre des systèmes Rust', () => {
  it('un système ajoute sa ligne à systems/mod.rs sans PR de contrat', () => {
    const r = check('feat(systems): mouvement', ['crates/lov-sim/src/systems/movement.rs', 'crates/lov-sim/src/systems/mod.rs']);
    expect(r.ok).toBe(true);
  });

  it('l’ordre des systèmes (registry.rs) reste un contrat', () => {
    const r = check('feat(systems): mouvement', ['crates/lov-sim/src/systems/movement.rs', 'crates/lov-sim/src/systems/registry.rs']);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('registry.rs');
  });
});

describe('check-areas : fonctionnalité qui traverse plusieurs areas', () => {
  const boutique = [
    'crates/lov-client/src/ui/hud/shop.rs',
    'crates/lov-client/src/ui/logic/shop.rs',
    'crates/lov-client/src/ui/icons/draw.rs',
  ];

  it('refuse deux areas quand une seule est déclarée', () => {
    const r = check('feat(ui-hud): boutique plus claire', boutique);
    expect(r.ok).toBe(false);
    expect(r.out).toContain('logic/shop.rs');
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
      'crates/lov-client/src/input/mod.rs',
      'crates/lov-client/src/ui/hud/mod.rs',
      'crates/lov-client/src/ui/mod.rs',
      'crates/lov-client/src/net/mod.rs',
      'crates/lov-client/src/main.rs',
    ]);
    expect(r.ok).toBe(true);
    expect(r.out).toContain('5 areas déclarées');
  });

  it('refuse toujours un contrat, même avec plusieurs areas', () => {
    const r = check('feat(ui-hud,ui-core): boutique', [...boutique, 'crates/lov-shared/src/schema.rs']);
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
      ['crates/lov-shared/src/data/structures.rs', 'feat(systems): plaques de tour'],
      ['crates/lov-shared/src/data/minions.rs', 'fix(systems): dégâts des sbires'],
      ['crates/lov-shared/src/data/monsters.rs', 'fix(world-ai): camps de jungle'],
      ['crates/lov-shared/src/data/items.rs', 'feat(items): nouvel objet'],
      ['crates/lov-shared/src/data/buffs.rs', 'feat(abilities): nouveau CC'],
      ['crates/lov-shared/src/data/summoners.rs', 'fix(abilities): sort d’invocateur'],
    ];
    for (const [file, title] of cases) expect(check(title, [file]).ok, `${title} → ${file}`).toBe(true);
  });
});
