#!/usr/bin/env node
// Libère les verrous dormants et réveille les PR qui dorment.
//
// Le modèle du dépôt tient sur « s'assigner une issue = poser un verrou sur ses fichiers »
// (CONTRIBUTING.md §3). Un verrou qu'on oublie de rendre bloque les autres en silence, et une PR
// qui attend prend du retard sur main puis conflicte. Ce script passe une fois par jour :
//
//   - issue assignée, sans activité depuis --issue-days (14) → commentaire + désassignation
//   - PR ouverte, sans activité depuis --pr-days (3)         → commentaire de rappel
//
// Le commentaire met à jour `updated_at`, donc rien ne se répète le lendemain.
//
//   usage : GH_TOKEN=$(gh auth token) node scripts/stale-locks.mjs [--dry-run] [--issue-days 14] [--pr-days 3]
const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  return i !== -1 && process.argv[i + 1] ? Number(process.argv[i + 1]) : fallback;
};
const dryRun = process.argv.includes('--dry-run');
const issueDays = arg('issue-days', 14);
const prDays = arg('pr-days', 3);
const repo = process.env.GITHUB_REPOSITORY ?? 'Primoux/league_of_vibes';
const token = process.env.GH_TOKEN ?? process.env.GITHUB_TOKEN;
if (!token) {
  console.error('GH_TOKEN manquant. Essaie : GH_TOKEN=$(gh auth token) node scripts/stale-locks.mjs --dry-run');
  process.exit(1);
}

const api = async (method, path, body) => {
  const res = await fetch(`https://api.github.com/repos/${repo}${path}`, {
    method,
    headers: { authorization: `Bearer ${token}`, accept: 'application/vnd.github+json', 'content-type': 'application/json' },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) throw new Error(`${method} ${path} → ${res.status}`);
  return res.json().catch(() => null);
};

const daysSince = (iso) => (Date.now() - Date.parse(iso)) / 86_400_000;
const act = async (what, fn) => {
  if (dryRun) return console.log(`  [dry-run] ${what}`);
  await fn();
  console.log(`  ${what}`);
};

const open = await api('GET', '/issues?state=open&per_page=100');
let freed = 0;
let nudged = 0;

for (const item of open) {
  const age = daysSince(item.updated_at);
  const n = item.number;

  // --- Pull requests : rappel quand ça dort ---
  if (item.pull_request) {
    if (age < prDays) continue;
    console.log(`PR #${n} — ${age.toFixed(0)} j sans activité — ${item.title}`);
    await act('commentaire de rappel', () =>
      api('POST', `/issues/${n}/comments`, {
        body:
          `Cette PR n'a pas bougé depuis ${Math.floor(age)} jours.\n\n` +
          `- **Auteur** : \`git fetch origin && git merge origin/main\` puis repousse — une PR qui attend prend du retard sur \`main\` et finit par conflicter.\n` +
          `- **Mainteneur** : elle attend une relecture. Si elle est trop grosse pour être relue, renvoie-la découpée sans la relire en détail (\`docs/REVIEW.md\`).\n` +
          `- Si elle est abandonnée, ferme-la et désassigne l'issue liée pour libérer le verrou.\n\n` +
          `<sub>Rappel automatique — voir \`CONTRIBUTING.md\` §1.</sub>`,
      }),
    );
    nudged++;
    continue;
  }

  // --- Issues : libération du verrou ---
  if (!item.assignees?.length || age < issueDays) continue;
  const who = item.assignees.map((a) => `@${a.login}`).join(', ');
  console.log(`Issue #${n} — ${age.toFixed(0)} j sans activité — assignée à ${who} — ${item.title}`);
  await act(`commentaire + désassignation de ${who}`, async () => {
    await api('POST', `/issues/${n}/comments`, {
      body:
        `${who} — cette tâche t'est assignée depuis ${Math.floor(age)} jours sans activité, je libère le verrou ` +
        `pour que quelqu'un d'autre puisse la prendre.\n\n` +
        `Tu es toujours dessus ? Réassigne-la toi : \`gh issue edit ${n} --add-assignee @me\`. ` +
        `Tu as du travail commencé ? Ouvre la PR même incomplète, en brouillon, plutôt que de le garder sur ta machine.\n\n` +
        `<sub>Libération automatique — voir \`CONTRIBUTING.md\` §3.</sub>`,
    });
    await api('DELETE', `/issues/${n}/assignees`, { assignees: item.assignees.map((a) => a.login) });
  });
  freed++;
}

console.log(`\n${freed} verrou(x) libéré(s), ${nudged} PR réveillée(s)${dryRun ? ' (dry-run)' : ''}.`);
