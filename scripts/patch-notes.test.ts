import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseFragment, render, extract, patchName } from './patch-notes.mjs';

describe('notes de patch', () => {
  it('fusionne deux fragments qui touchent le même champion, catégories dans l’ordre fixe', () => {
    const a = parseFragment('## Objets\n### Grande glaive\n- Prix corrigé\n\n## Champions\n### Corvin\n- Q : 60 ⇒ 70\n');
    const b = parseFragment('## Champions\n### Corvin\n- W : portée 600 ⇒ 650\n### Nyx\n- R : délai 1 s ⇒ 0,8 s\n');
    assert.equal(
      render([...a, ...b]),
      [
        '## Champions',
        '### Corvin',
        '- Q : 60 ⇒ 70',
        '- W : portée 600 ⇒ 650',
        '### Nyx',
        '- R : délai 1 s ⇒ 0,8 s',
        '## Objets',
        '### Grande glaive',
        '- Prix corrigé',
      ].join('\n'),
    );
  });

  it('les lignes sans sujet passent avant les sujets de leur catégorie', () => {
    const e = parseFragment('## Système\n### Tours\n- Ciblage\n## Système\n- Rappel interrompu par les sorts\n');
    assert.equal(render(e), '## Système\n- Rappel interrompu par les sorts\n### Tours\n- Ciblage');
  });

  it('refuse une catégorie inconnue ou une ligne hors format, avec le fichier et la ligne', () => {
    assert.throws(() => parseFragment('## Divers\n- x', 'a.md'), /a\.md:1 : catégorie « Divers » inconnue/);
    assert.throws(() => parseFragment('## Objets\ntexte libre', 'b.md'), /b\.md:2 : ligne hors format/);
    assert.throws(() => parseFragment('- orpheline', 'c.md'), /c\.md:1/);
  });

  it('extrait le texte d’une version de patchs.md', () => {
    const text = '# Notes\nintro\n\n# Patch 0.2 — 2026-12-05\n\n## Objets\n- b\n\n# Patch 0.1 — 2026-11-28\n\n## Champions\n- a\n';
    assert.equal(extract(text, '0.2.0'), '## Objets\n- b');
    assert.equal(extract(text, '0.1.0'), '## Champions\n- a');
    assert.equal(extract(text, '0.3.0'), null);
  });

  it('nomme le patch sans le « .0 » final', () => {
    assert.equal(patchName('0.1.0'), '0.1');
    assert.equal(patchName('1.2.4'), '1.2.4');
  });
});
