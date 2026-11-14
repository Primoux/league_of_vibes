#!/usr/bin/env node
// Crée/met à jour un label GitHub « area:<nom> » par area de .github/areas.json,
// pour que les labels des issues ne divergent jamais du découpage réel du projet.
//
//   usage : GH_TOKEN=$(gh auth token) node scripts/sync-labels.mjs [--dry-run]
//   en CI : GH_TOKEN=${{ secrets.GITHUB_TOKEN }} (permission issues: write)
import { readFileSync } from 'node:fs';

const dryRun = process.argv.includes('--dry-run');
const repo = process.env.GITHUB_REPOSITORY ?? 'Primoux/league_of_vibes';
const token = process.env.GH_TOKEN ?? process.env.GITHUB_TOKEN;
if (!token && !dryRun) {
  console.error('GH_TOKEN manquant. Essaie : GH_TOKEN=$(gh auth token) node scripts/sync-labels.mjs');
  process.exit(1);
}

const cfg = JSON.parse(readFileSync(new URL('../.github/areas.json', import.meta.url), 'utf8'));

/** Labels dérivés du découpage, plus ceux du processus. */
const wanted = new Map();
for (const [name, area] of Object.entries(cfg.areas)) {
  wanted.set(`area:${name}`, { color: 'BFD4F2', description: area.desc.slice(0, 100) });
}
wanted.set('contrat', { color: 'D93F0B', description: "Touche un contrat partagé : à valider avant d'écrire le code" });
wanted.set('bonne première tâche', { color: '7057FF', description: 'Petite tâche bien délimitée pour commencer' });
wanted.set('bug', { color: 'D73A4A', description: 'Quelque chose ne marche pas en jeu' });

if (dryRun) {
  for (const [name, l] of wanted) console.log(`${name}\t#${l.color}\t${l.description}`);
  process.exit(0);
}

const api = async (method, path, body) => {
  const res = await fetch(`https://api.github.com/repos/${repo}${path}`, {
    method,
    headers: {
      authorization: `Bearer ${token}`,
      accept: 'application/vnd.github+json',
      'content-type': 'application/json',
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  return { ok: res.ok, status: res.status, body: await res.json().catch(() => null) };
};

const existing = new Set();
for (let page = 1; ; page++) {
  const res = await api('GET', `/labels?per_page=100&page=${page}`);
  if (!res.ok) {
    console.error(`Lecture des labels impossible (${res.status}).`);
    process.exit(1);
  }
  for (const l of res.body) existing.add(l.name);
  if (res.body.length < 100) break;
}

let created = 0;
let updated = 0;
for (const [name, label] of wanted) {
  if (existing.has(name)) {
    const res = await api('PATCH', `/labels/${encodeURIComponent(name)}`, label);
    if (res.ok) updated++;
    else console.error(`✗ ${name} : ${res.status}`);
  } else {
    const res = await api('POST', '/labels', { name, ...label });
    if (res.ok) {
      created++;
      console.log(`+ ${name}`);
    } else console.error(`✗ ${name} : ${res.status}`);
  }
}

// Les labels d'area orphelins (une area renommée ou supprimée) sont signalés, jamais supprimés :
// des issues y sont peut-être encore rattachées.
const orphans = [...existing].filter((n) => n.startsWith('area:') && !wanted.has(n));
if (orphans.length) console.log(`⚠ labels d'area sans area correspondante : ${orphans.join(', ')}`);

console.log(`${created} créé(s), ${updated} à jour.`);
