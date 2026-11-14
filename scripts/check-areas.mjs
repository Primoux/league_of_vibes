#!/usr/bin/env node
// Vérifie qu'une PR reste dans le périmètre (« area ») annoncé par son titre.
//
//   usage : node scripts/check-areas.mjs [--title "feat(fx-core): ..."] [--base origin/main]
//           node scripts/check-areas.mjs --title "..." --files "a.ts,b.ts"   (pour les tests)
//
// Règles (voir AGENTS.md §2) :
//   1. Le titre suit le commit conventionnel : type(area): résumé — ou « contract: ... ».
//   2. Une PR normale ne touche que les chemins de son area (+ tests, + docs) et AUCUN fichier de contrat.
//   3. Une PR « contract: » ne touche QUE des fichiers de contrat (+ docs) : elle est mergée seule.
//   4. Une PR « champion » ne touche qu'un seul champion à la fois (tranche verticale).
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  return i !== -1 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
};

const cfg = JSON.parse(readFileSync(new URL('../.github/areas.json', import.meta.url), 'utf8'));
const title = arg('title', process.env.PR_TITLE ?? '');
const base = arg('base', process.env.BASE_REF ? `origin/${process.env.BASE_REF}` : 'origin/main');

/** Glob minimal : `**` = n'importe quels segments, `*` = un segment sans `/`. */
function matches(pattern, file) {
  const rx = pattern
    .split('/')
    .map((seg) => (seg === '**' ? '\u0000' : seg.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '[^/]*')))
    .join('/')
    .replace(/\u0000\//g, '(?:.*/)?')
    .replace(/\/?\u0000/g, '(?:/.*)?');
  return new RegExp(`^${rx}$`).test(file);
}
const matchesAny = (patterns, file) => patterns.some((p) => matches(p, file));

const fail = (msg) => {
  console.error(`\u001b[31m✗ ${msg}\u001b[0m`);
  process.exitCode = 1;
};

// --- fichiers modifiés par la PR ---
const explicit = arg('files', '');
let files;
if (explicit) {
  files = explicit.split(/[,\n]/).map((f) => f.trim()).filter(Boolean);
} else try {
  files = execFileSync('git', ['diff', '--name-only', `${base}...HEAD`], { encoding: 'utf8' })
    .split('\n')
    .filter(Boolean);
} catch {
  console.error(`Impossible de differ contre ${base} (git fetch --depth=0 manquant ?).`);
  process.exit(1);
}
if (files.length === 0) {
  console.log('Aucun fichier modifié.');
  process.exit(0);
}

// --- area annoncée par le titre ---
const areaNames = Object.keys(cfg.areas).sort();
const isContract = /^contract(\(|:)/.test(title);
const scope = title.match(/^[a-z]+\(([^)]+)\)!?:/)?.[1];

if (!isContract && !scope) {
  fail(
    `Titre de PR invalide : « ${title || '(vide)'} ».\n` +
      `  Attendu : type(area): résumé   — par exemple « feat(fx-core): halo de soin »\n` +
      `  ou bien : contract: résumé     — pour un changement de contrat partagé.\n` +
      `  Areas disponibles : ${areaNames.join(', ')}`,
  );
  process.exit(1);
}

const touchedContracts = files.filter((f) => cfg.contracts.includes(f));
const isDoc = (f) => matchesAny(cfg.always, f) || matchesAny(cfg.docs ?? [], f);
const isTest = (f) => matchesAny(cfg.tests, f);

// --- cas 1 : PR de contrat ---
if (isContract) {
  const strays = files.filter((f) => !cfg.contracts.includes(f) && !isDoc(f) && !isTest(f));
  if (strays.length) {
    fail(
      'Une PR « contract: » ne doit contenir que des fichiers de contrat (+ docs/tests), ' +
        'pour être relue et mergée seule. Hors périmètre :\n    ' +
        strays.join('\n    '),
    );
  }
  if (!touchedContracts.length) {
    fail('PR « contract: » qui ne touche aucun fichier de contrat. Utilise plutôt type(area): …');
  }
  if (!process.exitCode) console.log(`✓ PR de contrat : ${touchedContracts.length} fichier(s) partagé(s), rien d'autre.`);
  process.exit();
}

// --- cas 2 : PR normale ---
const area = cfg.areas[scope];
if (!area) {
  fail(`Area inconnue : « ${scope} ».\n  Areas disponibles : ${areaNames.join(', ')}\n  (ajoute la tienne dans .github/areas.json, dans une PR infra séparée)`);
  process.exit(1);
}

if (touchedContracts.length) {
  fail(
    'Fichiers de contrat partagés modifiés dans une PR normale :\n    ' +
      touchedContracts.join('\n    ') +
      '\n  Sors-les dans une PR dédiée « contract: … », validée avant celle-ci (AGENTS.md §2).',
  );
}

const strays = files.filter((f) => !matchesAny(area.paths, f) && !isDoc(f) && !isTest(f) && !cfg.contracts.includes(f));
if (strays.length) {
  fail(
    `Fichiers hors de l'area « ${scope} » (${area.desc}) :\n    ` +
      strays.join('\n    ') +
      `\n  Périmètre de « ${scope} » :\n    ` +
      area.paths.join('\n    ') +
      '\n  Découpe la PR par area : une PR = une area = une issue.',
  );
}

// --- cas 3 : tranche verticale « champion » : un seul champion par PR ---
if (area.vertical === 'champion') {
  const names = new Set();
  for (const f of files) {
    if (!matchesAny(area.paths, f)) continue;
    const name = f.split('/').pop().replace(/\.ts$/, '');
    if (name !== 'index' && !name.startsWith('_')) names.add(name);
  }
  if (names.size > 1) {
    fail(
      `Une PR « champion » ne couvre qu'un champion à la fois ; trouvés : ${[...names].join(', ')}.\n` +
        '  Ouvre une PR (et une issue) par champion : elles se mergent alors en parallèle sans conflit.',
    );
  }
  if (!process.exitCode && names.size === 1) console.log(`✓ Tranche verticale « ${[...names][0]} » : ${files.length} fichier(s).`);
}

if (!process.exitCode) console.log(`✓ PR dans le périmètre de « ${scope} » : ${files.length} fichier(s).`);
