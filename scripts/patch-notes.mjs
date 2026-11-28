#!/usr/bin/env node
// Notes de patch : assemble les fragments de `docs/patch-notes/next/` dans `docs/patch-notes/patchs.md`.
//
//   usage : node scripts/patch-notes.mjs assemble <version> [--date AAAA-MM-JJ]   fragments → patchs.md
//           node scripts/patch-notes.mjs extract <version>                        texte d'une version
//           node scripts/patch-notes.mjs check                                    fragments valides ?
//
// Chaque PR qui change quelque chose de visible pour le joueur ajoute SON fichier dans `next/` (aucun
// fichier partagé, donc aucun conflit). Format d'un fragment, voir `docs/patch-notes/README.md` :
//
//   ## Champions
//   ### Corvin
//   - Q : dégâts 60/95/130 ⇒ 70/105/140
//
// À la publication, les fragments sont fusionnés par catégorie puis par sujet, dans l'ordre des
// catégories ci-dessous, et la version est ajoutée en tête de `patchs.md` (le client l'affiche).
import { readFileSync, writeFileSync, readdirSync, rmSync, existsSync } from 'node:fs';
import { join } from 'node:path';

export const CATEGORIES = ['Champions', 'Objets', 'Jungle et objectifs', 'Système', 'Interface', 'Correctifs'];

const ROOT = new URL('../docs/patch-notes/', import.meta.url).pathname;
const NEXT = join(ROOT, 'next');
const PATCHS = join(ROOT, 'patchs.md');

/** Version affichée aux joueurs : « 0.3.0 » → « 0.3 », « 1.2.4 » → « 1.2.4 ». */
export function patchName(version) {
  return version.replace(/\.0$/, '');
}

/**
 * Lit un fragment : `## Catégorie`, `### Sujet` (facultatif), puis des puces `- …`.
 * Renvoie `[{ category, subject, line }]` ; lève une erreur lisible sur une ligne hors format.
 */
export function parseFragment(text, file = 'fragment') {
  const out = [];
  let category = null;
  let subject = '';
  text.split('\n').forEach((raw, i) => {
    const line = raw.trimEnd();
    const where = `${file}:${i + 1}`;
    if (!line.trim() || line.startsWith('<!--')) return;
    let m;
    if ((m = line.match(/^## (.+)$/))) {
      category = m[1].trim();
      if (!CATEGORIES.includes(category)) {
        throw new Error(`${where} : catégorie « ${category} » inconnue (${CATEGORIES.join(', ')})`);
      }
      subject = '';
    } else if ((m = line.match(/^### (.+)$/))) {
      if (!category) throw new Error(`${where} : un sujet ### doit suivre une catégorie ##`);
      subject = m[1].trim();
    } else if ((m = line.match(/^\s*[-*] (.+)$/))) {
      if (!category) throw new Error(`${where} : une ligne doit suivre une catégorie ##`);
      out.push({ category, subject, line: m[1].trim() });
    } else {
      throw new Error(`${where} : ligne hors format (attendu « ## », « ### » ou « - ») : ${line}`);
    }
  });
  return out;
}

/** Fusionne des entrées en Markdown : catégories dans l'ordre fixe, sujets dans l'ordre d'apparition. */
export function render(entries) {
  const parts = [];
  for (const category of CATEGORIES) {
    const own = entries.filter((e) => e.category === category);
    if (!own.length) continue;
    parts.push(`## ${category}`);
    const subjects = [...new Set(own.map((e) => e.subject))].sort((a, b) => (a === '' ? -1 : b === '' ? 1 : 0));
    for (const subject of subjects) {
      if (subject) parts.push(`### ${subject}`);
      for (const e of own.filter((x) => x.subject === subject)) parts.push(`- ${e.line}`);
    }
  }
  return parts.join('\n');
}

/** Fragments de `next/` (hors README), triés par nom pour un résultat stable. */
function fragments() {
  if (!existsSync(NEXT)) return [];
  return readdirSync(NEXT)
    .filter((f) => f.endsWith('.md') && f !== 'README.md')
    .sort()
    .map((f) => ({ file: f, path: join(NEXT, f) }));
}

/** En-tête de `patchs.md` : tout ce qui précède la première version. */
function splitPatchs(text) {
  const i = text.search(/^# Patch /m);
  return i === -1 ? [text.trimEnd() + '\n\n', ''] : [text.slice(0, i), text.slice(i)];
}

/** Section d'une version dans `patchs.md` (sans son titre), ou `null`. */
export function extract(text, version) {
  const name = patchName(version);
  const sections = text.split(/^(?=# Patch )/m);
  const own = sections.find((s) => s.match(/^# Patch (\S+)/)?.[1] === name);
  return own ? own.replace(/^# Patch .*\n/, '').trim() : null;
}

function main() {
  const [cmd, version] = process.argv.slice(2);
  const dateArg = process.argv.indexOf('--date');
  const date = dateArg !== -1 ? process.argv[dateArg + 1] : new Date().toISOString().slice(0, 10);

  if (cmd === 'check') {
    for (const f of fragments()) parseFragment(readFileSync(f.path, 'utf8'), `next/${f.file}`);
    console.log(`✓ ${fragments().length} fragment(s) de notes de patch valides.`);
    return;
  }
  if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
    console.error('usage : patch-notes.mjs assemble|extract <version x.y.z> [--date AAAA-MM-JJ]');
    process.exit(2);
  }
  const patchs = existsSync(PATCHS) ? readFileSync(PATCHS, 'utf8') : '';
  if (cmd === 'extract') {
    const body = extract(patchs, version);
    if (body === null) {
      console.error(`Aucune note pour le patch ${patchName(version)} dans docs/patch-notes/patchs.md`);
      process.exit(1);
    }
    console.log(body);
    return;
  }
  if (cmd === 'assemble') {
    if (extract(patchs, version) !== null) {
      console.error(`Le patch ${patchName(version)} existe déjà dans patchs.md`);
      process.exit(1);
    }
    const files = fragments();
    const entries = files.flatMap((f) => parseFragment(readFileSync(f.path, 'utf8'), `next/${f.file}`));
    const body = entries.length ? render(entries) : '## Système\n- Corrections et améliorations diverses.';
    const [head, rest] = splitPatchs(patchs);
    writeFileSync(PATCHS, `${head}# Patch ${patchName(version)} — ${date}\n\n${body}\n\n${rest}`.trimEnd() + '\n');
    for (const f of files) rmSync(f.path);
    console.log(`✓ Patch ${patchName(version)} : ${entries.length} ligne(s), ${files.length} fragment(s) consommé(s).`);
    return;
  }
  console.error(`commande inconnue : ${cmd}`);
  process.exit(2);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
