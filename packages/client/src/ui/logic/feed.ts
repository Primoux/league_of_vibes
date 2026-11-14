/**
 * Transforme les GameEvent en entrées de kill feed et en annonces centrales (pur, testé).
 * Les annonces de kills (premier sang, multi-kills, séries, arrêt de série, équipe anéantie),
 * de structures et d'objectifs sont dérivées ici. Les événements `announcement` du serveur sont
 * affichés tels quels via ANNOUNCE_TEXT (clés libres : minionsSpawn, leviathanSpawn...).
 */
import type { DragonElement, EntityId, EntitySnapshot, GameEvent, MonsterType, Team } from '@lov/shared';
import { DRAGON_LABEL, MONSTER_LABEL, SOUL_LABEL } from './format.ts';

/** Ce que l'UI sait d'une unité pour l'afficher. */
export interface UnitRef {
  id: EntityId;
  kind: EntitySnapshot['kind'] | 'unknown';
  team: Team | 'neutral';
  /** championId, type de sbire/monstre, palier de tour... */
  subtype?: string;
  name?: string;
}

export type FeedKind = 'kill' | 'execute' | 'structure' | 'objective';

export interface FeedEntry {
  key: number;
  time: number; // secondes de jeu
  kind: FeedKind;
  killer?: UnitRef;
  victim?: UnitRef;
  assists: UnitRef[];
  /** Équipe « bénéficiaire » (pour la couleur). */
  team: Team | 'neutral';
  label?: string;
  element?: DragonElement;
  monster?: MonsterType;
}

export type AnnounceTone = 'ally' | 'enemy' | 'neutral' | 'gold';

export interface Announcement {
  key: number;
  title: string;
  subtitle?: string;
  tone: AnnounceTone;
  killer?: UnitRef;
  victim?: UnitRef;
  /** Priorité : une annonce plus prioritaire passe devant la file. */
  priority: number;
  element?: DragonElement;
}

export const ANNOUNCE_TEXT: Record<string, string> = {
  welcome: 'Bienvenue dans la Faille des Vibes',
  minionsSoon: 'Les sbires apparaîtront dans 30 secondes',
  minionsSpawn: 'Les sbires sont apparus !',
  dragonSpawn: 'Un drake est apparu',
  elderSpawn: 'Le Drake Ancien s’est réveillé',
  grubsSpawn: 'Les Larves ont éclos',
  heraldSpawn: 'Le Colosse du Rift est apparu',
  leviathanSpawn: 'Le Léviathan a émergé des profondeurs',
  inhibitorRespawn: 'Un inhibiteur s’est reconstruit',
  dragonSoul: 'Âme du drake obtenue',
  // clés émises par codex-objectives
  herald_summoned: 'Le Colosse du Rift charge !',
  inhibitor_respawn: 'Un inhibiteur s’est reconstruit',
};

/** Clés serveur volontairement ignorées : déjà dérivées des événements (doublons) ou gérées par l'écran de fin. */
const SKIPPED_KEYS = new Set(['objective_taken', 'victory', 'defeat', 'first_blood', 'kill', 'multikill', 'ace']);

const MULTI = ['', '', 'Double élimination !', 'Triple élimination !', 'Quadra élimination !', 'Penta élimination !'];
const SPREE: Record<number, string> = { 3: 'Série meurtrière', 4: 'Massacre', 5: 'Déchaîné', 6: 'Inarrêtable', 7: 'Divin', 8: 'Légendaire' };

const MULTI_WINDOW = 10; // secondes entre deux kills d'une même série
const TEAM_SIZE = 4;

export class FeedTracker {
  private nextKey = 1;
  private firstBlood = false;
  private streak = new Map<EntityId, number>(); // kills sans mourir (champions)
  private multi = new Map<EntityId, { count: number; last: number }>();
  /** Champions morts connus (pour « Équipe anéantie »). */
  private deadUntilRespawn = new Map<EntityId, Team>();

  constructor(
    private myTeam: Team,
    private lookup: (id: EntityId) => UnitRef,
  ) {}

  setTeam(t: Team): void {
    this.myTeam = t;
  }

  markAlive(id: EntityId): void {
    this.deadUntilRespawn.delete(id);
  }

  process(events: readonly GameEvent[], time: number): { feed: FeedEntry[]; announce: Announcement[] } {
    const feed: FeedEntry[] = [];
    const announce: Announcement[] = [];
    for (const e of events) {
      switch (e.type) {
        case 'death':
          this.onDeath(e, time, feed, announce);
          break;
        case 'structureDestroyed': {
          if (e.plate) break;
          const victim = this.lookup(e.target);
          const allyLost = e.team === this.myTeam;
          const label = e.kind === 'tower' ? 'Tour' : e.kind === 'inhibitor' ? 'Inhibiteur' : 'Nexus';
          feed.push({ key: this.nextKey++, time, kind: 'structure', victim: { ...victim, team: e.team, kind: e.kind }, assists: [], team: allyLost ? enemyOf(this.myTeam) : this.myTeam, label });
          if (e.kind !== 'nexus') {
            announce.push({
              key: this.nextKey++,
              title: e.kind === 'tower' ? (allyLost ? 'Votre tour a été détruite' : 'Tour ennemie détruite !') : allyLost ? 'Votre inhibiteur a été détruit' : 'Inhibiteur ennemi détruit !',
              subtitle: e.kind === 'inhibitor' ? (allyLost ? 'Des super-sbires vont déferler' : 'Vos super-sbires déferlent') : undefined,
              tone: allyLost ? 'enemy' : 'ally',
              priority: e.kind === 'inhibitor' ? 3 : 2,
            });
          }
          break;
        }
        case 'objectiveTaken': {
          const ally = e.team === this.myTeam;
          const name = objectiveName(e.monster, e.element);
          const killer = e.killer !== undefined ? this.lookup(e.killer) : undefined;
          feed.push({ key: this.nextKey++, time, kind: 'objective', killer, assists: [], team: e.team, label: name, element: e.element, monster: e.monster });
          if (e.monster === 'grub') break; // pas d'annonce par larve, juste le feed
          const verb = e.monster === 'herald' ? 'invoqué' : 'vaincu';
          announce.push({
            key: this.nextKey++,
            title: ally ? `Votre équipe a ${verb} : ${name}` : `L’équipe ennemie a ${verb} : ${name}`,
            tone: ally ? 'ally' : 'enemy',
            priority: e.monster === 'leviathan' || e.monster === 'elder_dragon' ? 4 : 3,
            element: e.element,
            killer,
          });
          break;
        }
        case 'announcement': {
          if (SKIPPED_KEYS.has(e.key)) break;
          if (e.key === 'dragon_soul' || e.key === 'dragonSoul') {
            const el = e.params?.element;
            const ally = e.team === this.myTeam;
            const soul = typeof el === 'string' && el in SOUL_LABEL ? `Âme ${SOUL_LABEL[el as DragonElement]}` : 'Âme du drake';
            announce.push({ key: this.nextKey++, title: ally ? `Votre équipe obtient l’${soul} !` : `L’équipe ennemie obtient l’${soul}`, subtitle: 'Le Drake Ancien se réveillera bientôt', tone: ally ? 'ally' : 'enemy', priority: 6, element: typeof el === 'string' && el in SOUL_LABEL ? (el as DragonElement) : undefined });
            break;
          }
          const title = ANNOUNCE_TEXT[e.key] ?? (typeof e.params?.text === 'string' ? e.params.text : null);
          if (!title) break;
          const tone: AnnounceTone = e.team ? (e.team === this.myTeam ? 'ally' : 'enemy') : 'neutral';
          announce.push({ key: this.nextKey++, title, tone, priority: 1 });
          break;
        }
        default:
          break;
      }
    }
    return { feed, announce };
  }

  private onDeath(e: Extract<GameEvent, { type: 'death' }>, time: number, feed: FeedEntry[], announce: Announcement[]): void {
    const victim = this.lookup(e.target);
    if (victim.kind !== 'champion') return;
    const killer = e.killer !== undefined ? this.lookup(e.killer) : undefined;
    const assists = (e.assists ?? []).map((a) => this.lookup(a));
    const victimTeam = victim.team === 'neutral' ? this.myTeam : victim.team;
    const killerIsChamp = killer?.kind === 'champion';
    const allyKilled = victimTeam === this.myTeam;

    const victimStreak = this.streak.get(victim.id) ?? 0;
    this.streak.set(victim.id, 0);
    this.multi.delete(victim.id);
    this.deadUntilRespawn.set(victim.id, victimTeam);

    feed.push({ key: this.nextKey++, time, kind: killerIsChamp ? 'kill' : 'execute', killer, victim, assists, team: enemyOf(victimTeam) });

    const tone: AnnounceTone = allyKilled ? 'enemy' : 'ally';
    const base = { killer, victim, tone } as const;

    if (!killerIsChamp || !killer) {
      announce.push({ ...base, key: this.nextKey++, title: allyKilled ? 'Un allié a été exécuté' : 'Un ennemi a été exécuté', priority: 1 });
      this.checkAce(victimTeam, announce);
      return;
    }

    const streak = (this.streak.get(killer.id) ?? 0) + 1;
    this.streak.set(killer.id, streak);
    const m = this.multi.get(killer.id);
    const multi = m && time - m.last <= MULTI_WINDOW ? m.count + 1 : 1;
    this.multi.set(killer.id, { count: multi, last: time });

    if (!this.firstBlood) {
      this.firstBlood = true;
      announce.push({ ...base, key: this.nextKey++, title: 'Premier sang !', subtitle: who(killer, victim, allyKilled), tone: 'gold', priority: 5 });
    } else if (multi >= 2) {
      announce.push({ ...base, key: this.nextKey++, title: MULTI[Math.min(multi, 5)] ?? 'Élimination !', subtitle: who(killer, victim, allyKilled), priority: 4 + multi });
    } else if (victimStreak >= 3) {
      announce.push({ ...base, key: this.nextKey++, title: 'Arrêt de série !', subtitle: `${victim.name ?? 'Un champion'} ne fera plus de ravages`, priority: 4 });
    } else {
      announce.push({ ...base, key: this.nextKey++, title: allyKilled ? 'Un allié a été tué' : 'Vous avez éliminé un ennemi', subtitle: who(killer, victim, allyKilled), priority: 2 });
    }
    if (streak >= 3 && multi < 2) {
      const title = SPREE[Math.min(streak, 8)] ?? 'Légendaire';
      const enemy = killer.team !== this.myTeam;
      announce.push({ ...base, key: this.nextKey++, title: enemy && streak >= 8 ? 'Un ennemi est légendaire !' : `${killer.name ?? 'Un champion'} : ${title} !`, tone: enemy ? 'enemy' : 'ally', priority: 3 });
    }
    this.checkAce(victimTeam, announce);
  }

  private checkAce(team: Team, announce: Announcement[]): void {
    let dead = 0;
    this.deadUntilRespawn.forEach((t) => {
      if (t === team) dead++;
    });
    if (dead >= TEAM_SIZE) {
      announce.push({ key: this.nextKey++, title: 'Équipe anéantie !', tone: team === this.myTeam ? 'enemy' : 'ally', priority: 9 });
    }
  }
}

const enemyOf = (t: Team): Team => (t === 'blue' ? 'red' : 'blue');

function who(killer: UnitRef, victim: UnitRef, _allyKilled: boolean): string {
  return `${killer.name ?? 'Un champion'} a abattu ${victim.name ?? 'un champion'}`;
}

export function objectiveName(monster: MonsterType, element?: DragonElement): string {
  if (monster === 'dragon' && element) return DRAGON_LABEL[element];
  return MONSTER_LABEL[monster] ?? monster;
}

/** Garde les `max` entrées les plus récentes et retire celles plus vieilles que `ttl` secondes. */
export function trimFeed<T extends { time: number }>(entries: readonly T[], now: number, max = 6, ttl = 14): T[] {
  return entries.filter((e) => now - e.time <= ttl).slice(-max);
}
