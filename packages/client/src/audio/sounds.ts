/**
 * Table de correspondance GameEvent -> sons synthétisés. Propriétaire : mistral-economy.
 * Chaque `fxKey` des 40 sorts a une recette procédurale (dérivée du champion et du slot),
 * les sorts d'invocateur ont leur propre table, et tout événement a un fallback par type.
 */
import { CHAMPION_LIST, SUMMONERS, type GameEvent, type SummonerId, type Team } from '@lov/shared';
import { store } from '../state.ts';
import type { AudioManager, SfxFn } from './AudioManager.ts';
import { KillStreaks, stingLevel } from './killStreaks.ts';
import { MusicBus } from './music/instruments.ts';
import { firstBloodSting, killSting, type StingSide } from './music/stings.ts';
import { noise, tone } from './synth.ts';

// ---------------------------------------------------------------- recettes par fxKey
type Recipe = (a: AudioManager, pos: { x: number; y: number } | undefined, vol?: number) => void;
const fxRecipes = new Map<string, Recipe>();

function note(base: number, semis: number): number {
  return base * Math.pow(2, semis / 12);
}

// Un sort par champion/slot : timbre et hauteur dérivés de l'index du champion et du slot,
// ce qui couvre les 40 sorts de façon cohérente, avec fallback générique par slot.
CHAMPION_LIST.forEach((c, ci) => {
  const base = note(196, (ci * 3) % 12); // chaque champion a sa couleur fréquentielle
  const slots: Record<string, { semis: number; type: OscillatorType; dur: number }> = {
    Q: { semis: 0, type: 'square', dur: 0.18 },
    W: { semis: 4, type: 'sawtooth', dur: 0.22 },
    E: { semis: 7, type: 'triangle', dur: 0.25 },
    R: { semis: 12, type: 'sawtooth', dur: 0.5 },
  };
  for (const [slot, cfg] of Object.entries(slots)) {
    const a = c.abilities[slot as keyof typeof c.abilities];
    fxRecipes.set(a.fxKey, (am, pos, vol) => {
      am.sfx((ctx, dest) => {
        tone(ctx, dest, { type: cfg.type, freq: base * Math.pow(2, cfg.semis / 12), slide: 1.5, dur: cfg.dur, vol: (vol ?? 0.25) * 0.7, filter: 2400 });
        noise(ctx, dest, { dur: cfg.dur * 0.6, vol: (vol ?? 0.25) * 0.25, filter: 3000, slideFilter: 600 });
      }, { pos });
    });
  }
  fxRecipes.set(c.passive.fxKey, (am, pos, vol) => {
    am.sfx((ctx, dest) => tone(ctx, dest, { type: 'sine', freq: base, slide: 0.5, dur: 0.4, vol: (vol ?? 0.2) * 0.5 }), { pos });
  });
});

// ---------------------------------------------------------------- recettes dédiées (Ults d'abord)
// On enregistre ici les sons sur mesure des ults, par fxKey ; ils priment sur la recette
// procédurale du champion. `customFxKeys` permet aussi de réserver l'événement areaEffect.
const customRecipes = new Map<string, Recipe>();
const customFxKeys = new Set<string>();

// Mélenchon — R « La République, c'est moi ! » : déclaration solennelle (0,6 s) puis onde
// tricolore (bleu #203452 / rouge #ef3340 / cyan #5de2ff, la palette du champion).
customFxKeys.add('melenchon_r');
customRecipes.set('melenchon_r', (am, pos, vol = 1) => {
  am.sfx((ctx, d) => {
    // 0 -> 0,6 s : montée grave de la déclaration
    tone(ctx, d, { type: 'sawtooth', freq: 110, slide: 220, dur: 0.6, vol: 0.22 * vol, filter: 900 });
    tone(ctx, d, { type: 'square', freq: 55, slide: 110, dur: 0.6, vol: 0.18 * vol, filter: 500 });
    // 0,6 s : impact de l'onde
    noise(ctx, d, { dur: 0.9, vol: 0.45 * vol, filter: 500, slideFilter: 60, delay: 0.6 });
    tone(ctx, d, { type: 'sine', freq: 55, dur: 1.0, vol: 0.5 * vol, delay: 0.6 });
    // motif tricolore
    tone(ctx, d, { type: 'square', freq: 220, dur: 0.18, vol: 0.2 * vol, delay: 0.6, filter: 2400 }); // bleu
    tone(ctx, d, { type: 'square', freq: 277, dur: 0.18, vol: 0.2 * vol, delay: 0.72, filter: 2400 }); // rouge
    tone(ctx, d, { type: 'square', freq: 554, dur: 0.32, vol: 0.22 * vol, delay: 0.84, filter: 3200 }); // cyan
    // envolée aiguë
    tone(ctx, d, { type: 'sine', freq: 1108, slide: 1660, dur: 0.35, vol: 0.12 * vol, delay: 0.6 });
  }, { pos });
});

const SUMMONER_RECIPES: Record<SummonerId, Recipe> = {
  flash: (am, pos) => am.sfx((ctx, d) => { tone(ctx, d, { type: 'square', freq: 1200, slide: 300, dur: 0.12, vol: 0.3 }); noise(ctx, d, { dur: 0.15, vol: 0.2, filter: 5000, slideFilter: 800 }); }, { pos }),
  ignite: (am, pos) => am.sfx((ctx, d) => { noise(ctx, d, { dur: 0.5, vol: 0.3, filter: 1800, slideFilter: 300 }); tone(ctx, d, { type: 'sawtooth', freq: 180, slide: 90, dur: 0.5, vol: 0.25, filter: 900 }); }, { pos }),
  heal: (am, pos) => am.sfx((ctx, d) => { tone(ctx, d, { freq: 520, slide: 1.5, dur: 0.4, vol: 0.25 }); tone(ctx, d, { freq: 780, slide: 1.4, dur: 0.4, vol: 0.2, delay: 0.08 }); }, { pos }),
  barrier: (am, pos) => am.sfx((ctx, d) => { tone(ctx, d, { type: 'triangle', freq: 300, slide: 600, dur: 0.5, vol: 0.3 }); }, { pos }),
  exhaust: (am, pos) => am.sfx((ctx, d) => { tone(ctx, d, { type: 'sawtooth', freq: 220, slide: 0.6, dur: 0.4, vol: 0.25, filter: 700 }); }, { pos }),
  teleport: (am, pos) => am.sfx((ctx, d) => { tone(ctx, d, { type: 'sine', freq: 400, slide: 2, dur: 0.8, vol: 0.25 }); tone(ctx, d, { freq: 800, slide: 2, dur: 0.8, vol: 0.15, delay: 0.1 }); }, { pos }),
  smite: (am, pos) => am.sfx((ctx, d) => { tone(ctx, d, { type: 'square', freq: 90, slide: 40, dur: 0.3, vol: 0.35, filter: 500 }); noise(ctx, d, { dur: 0.25, vol: 0.3, filter: 1200 }); }, { pos }),
  ghost: (am, pos) => am.sfx((ctx, d) => { noise(ctx, d, { dur: 0.6, vol: 0.2, filter: 2500, type: 'highpass' }); tone(ctx, d, { freq: 600, slide: 900, dur: 0.6, vol: 0.15 }); }, { pos }),
  cleanse: (am, pos) => am.sfx((ctx, d) => { tone(ctx, d, { type: 'triangle', freq: 700, slide: 1200, dur: 0.3, vol: 0.28 }); tone(ctx, d, { freq: 1050, dur: 0.25, vol: 0.2, delay: 0.1 }); }, { pos }),
};

// ---------------------------------------------------------------- annonces vocales
// Les éliminations n'ont plus de voix (jingles ci-dessous). Les `announcement` génériques passent par params.text.
const ANNOUNCEMENTS: Record<string, (p?: Record<string, string | number>) => string> = {
  colosse_invoque: () => 'Le Colosse du Rift charge !',
};

/**
 * Éliminations : uniquement champion tué par un champion ennemi (jamais les sbires, monstres ou tours).
 * Plus de voix : un jingle synthétisé qui gagne en ampleur avec le multi-kill et la série (killStreaks.ts).
 */
const streaks = new KillStreaks();
/** Équipe de chaque champion vu (les ids restent connus même quand l'unité passe dans le brouillard). */
const champTeams = new Map<number, Team>();
let stingBus: MusicBus | null = null;

export function resetKillAnnouncements(): void {
  streaks.reset();
  champTeams.clear();
}

function refreshChampions(): void {
  for (const e of store.state.snapshot?.entities ?? []) if (e.kind === 'champion' && e.team !== 'neutral') champTeams.set(e.id, e.team);
}

function getStingBus(a: AudioManager): MusicBus | null {
  if (!stingBus)
    a.sfx((ctx, dest) => {
      stingBus = new MusicBus(ctx, dest);
      stingBus.master.gain.value = 1;
    }, { channel: 'voice' });
  return stingBus;
}

function onDeath(a: AudioManager, victimId: number, killerId: number | undefined): void {
  refreshChampions();
  const victimTeam = champTeams.get(victimId);
  if (!victimTeam) return; // pas un champion : aucune annonce
  const killerTeam = killerId !== undefined ? champTeams.get(killerId) : undefined;
  if (killerId === undefined || !killerTeam || killerTeam === victimTeam) {
    streaks.onChampionDeath(victimId); // exécuté par une tour / un sbire / un monstre
    return;
  }
  const info = streaks.onChampionKill(victimId, killerId, performance.now() / 1000);
  const me = store.state.snapshot?.self?.entityId ?? store.state.game?.entityId;
  const myTeam = store.state.game?.team;
  const side: StingSide = killerId === me ? 'self' : killerTeam === myTeam ? 'ally' : 'enemy';
  const bus = getStingBus(a);
  if (!bus) return;
  const t = bus.ctx.currentTime + 0.05;
  if (info.firstBlood) firstBloodSting(bus, t, side);
  else killSting(bus, t, stingLevel(info), side);
}

function speak(text: string, voiceVol: number): void {
  try {
    if (typeof speechSynthesis === 'undefined' || voiceVol <= 0) return;
    const u = new SpeechSynthesisUtterance(text);
    u.lang = 'fr-FR';
    u.rate = 1.05;
    u.volume = voiceVol;
    speechSynthesis.speak(u);
  } catch { /* synthèse vocale indisponible */ }
}

// ---------------------------------------------------------------- événements -> sons
export function playEvent(a: AudioManager, e: GameEvent): void {
  const selfId = store.state.game?.entityId;
  switch (e.type) {
    case 'autoAttack':
      a.sfx((ctx, d) => {
        if (e.ranged) noise(ctx, d, { dur: 0.08, vol: 0.15, filter: 3500, slideFilter: 1200 });
        else { noise(ctx, d, { dur: 0.07, vol: 0.22, filter: 900 }); tone(ctx, d, { type: 'square', freq: 140, slide: 90, dur: 0.06, vol: 0.12 }); }
      }, { pos: a.entityPos(e.source) });
      break;

    case 'damage': {
      const vol = e.crit ? 0.3 : 0.18;
      a.sfx((ctx, d) => {
        if (e.damageType === 'magic') tone(ctx, d, { type: 'sawtooth', freq: 700, slide: 250, dur: 0.12, vol, filter: 2000 });
        else if (e.damageType === 'true') { tone(ctx, d, { type: 'square', freq: 300, slide: 120, dur: 0.1, vol }); noise(ctx, d, { dur: 0.08, vol: vol * 0.7, filter: 2500 }); }
        else { noise(ctx, d, { dur: 0.08, vol, filter: 1100 }); tone(ctx, d, { type: 'square', freq: 180, slide: 100, dur: 0.08, vol: vol * 0.8 }); }
      }, { pos: a.entityPos(e.target) });
      break;
    }

    case 'heal':
      a.sfx((ctx, d) => tone(ctx, d, { type: 'sine', freq: 620, slide: 930, dur: 0.25, vol: 0.15 }), { pos: a.entityPos(e.target) });
      break;
    case 'shield':
      a.sfx((ctx, d) => tone(ctx, d, { type: 'triangle', freq: 440, slide: 660, dur: 0.2, vol: 0.15 }), { pos: a.entityPos(e.target) });
      break;

    case 'death':
      a.sfx((ctx, d) => {
        noise(ctx, d, { dur: 0.5, vol: 0.3, filter: 800, slideFilter: 120 });
        tone(ctx, d, { type: 'sawtooth', freq: 160, slide: 50, dur: 0.5, vol: 0.25, filter: 600 });
      }, { pos: a.entityPos(e.target) });
      onDeath(a, e.target, e.killer);
      break;

    case 'respawn':
      a.sfx((ctx, d) => { tone(ctx, d, { freq: 523, dur: 0.3, vol: 0.2 }); tone(ctx, d, { freq: 784, dur: 0.35, vol: 0.18, delay: 0.12 }); }, { pos: a.entityPos(e.target) });
      break;

    case 'levelUp':
      a.sfx((ctx, d) => {
        tone(ctx, d, { freq: 523, dur: 0.15, vol: 0.22 });
        tone(ctx, d, { freq: 659, dur: 0.15, vol: 0.22, delay: 0.1 });
        tone(ctx, d, { freq: 784, dur: 0.3, vol: 0.25, delay: 0.2 });
      }, { pos: a.entityPos(e.target), channel: e.target === selfId ? 'sfx' : 'sfx', vol: e.target === selfId ? 1 : 0.7 });
      break;

    case 'goldGain':
      if (e.target === selfId) {
        a.sfx((ctx, d) => {
          tone(ctx, d, { type: 'square', freq: 1320, dur: 0.06, vol: 0.12 });
          tone(ctx, d, { type: 'square', freq: 1760, dur: 0.09, vol: 0.12, delay: 0.05 });
        }, { channel: 'ui' });
      }
      break;

    case 'itemBought':
      if (e.target === selfId) a.sfx((ctx, d) => { tone(ctx, d, { type: 'square', freq: 880, dur: 0.07, vol: 0.18 }); tone(ctx, d, { type: 'square', freq: 1320, dur: 0.12, vol: 0.18, delay: 0.06 }); }, { channel: 'ui' });
      break;
    case 'itemSold':
      if (e.target === selfId) a.sfx((ctx, d) => tone(ctx, d, { type: 'square', freq: 660, slide: 440, dur: 0.15, vol: 0.15 }), { channel: 'ui' });
      break;
    case 'itemUsed':
      a.sfx((ctx, d) => {
        if (e.itemId === 'health_potion') noise(ctx, d, { dur: 0.3, vol: 0.15, filter: 900, type: 'bandpass' });
        else tone(ctx, d, { type: 'triangle', freq: 500, slide: 750, dur: 0.2, vol: 0.2 });
      }, { pos: a.entityPos(e.target) });
      break;

    case 'recallStart':
      a.sfx((ctx, d) => tone(ctx, d, { type: 'sine', freq: 300, slide: 900, dur: 1.2, vol: 0.18 }), { pos: a.entityPos(e.target) });
      break;
    case 'recallEnd':
      a.sfx((ctx, d) => {
        if (e.completed) { tone(ctx, d, { type: 'sine', freq: 880, slide: 1320, dur: 0.3, vol: 0.22 }); tone(ctx, d, { freq: 1760, dur: 0.25, vol: 0.15, delay: 0.15 }); }
        else tone(ctx, d, { type: 'square', freq: 400, slide: 200, dur: 0.2, vol: 0.15 });
      }, { pos: a.entityPos(e.target) });
      break;

    case 'abilityCast': {
      const key = `${e.championId}_${e.slot}`;
      const recipe = customRecipes.get(key) ?? fxRecipes.get(key) ?? fxRecipes.get(`${e.championId}_${e.slot.toLowerCase()}`);
      const pos = a.entityPos(e.source) ?? e.targetPos;
      if (recipe) recipe(a, pos, 1);
      else a.sfx((ctx, d) => tone(ctx, d, { type: 'square', freq: 440, slide: 660, dur: 0.15, vol: 0.2 }), { pos });
      break;
    }

    case 'summonerCast': {
      const recipe = SUMMONER_RECIPES[e.spell];
      const pos = a.entityPos(e.source) ?? e.from;
      if (recipe) recipe(a, pos);
      break;
    }

    case 'projectileHit':
      a.sfx((ctx, d) => { noise(ctx, d, { dur: 0.1, vol: 0.2, filter: 2200, slideFilter: 500 }); tone(ctx, d, { freq: 500, slide: 200, dur: 0.1, vol: 0.12 }); }, { pos: e.pos });
      break;

    case 'dash':
      a.sfx((ctx, d) => noise(ctx, d, { dur: Math.min(0.4, e.duration), vol: 0.2, filter: 2000, type: 'bandpass', slideFilter: 500 }), { pos: e.from });
      break;

    case 'areaEffect':
      // les fxKey avec recette dédiée gèrent déjà leur impact (ex. ult de Mélenchon)
      if (!customFxKeys.has(e.fxKey)) {
        a.sfx((ctx, d) => { tone(ctx, d, { type: 'sawtooth', freq: 220, slide: 440, dur: Math.min(0.6, e.duration), vol: 0.15, filter: 1500 }); }, { pos: e.pos });
      }
      break;

    case 'buffApplied':
    case 'buffRemoved':
      a.sfx((ctx, d) => tone(ctx, d, { type: 'sine', freq: e.type === 'buffApplied' ? 700 : 350, slide: e.type === 'buffApplied' ? 1.3 : 0.7, dur: 0.15, vol: 0.1 }), { pos: a.entityPos(e.target) });
      break;

    case 'structureDestroyed':
      a.sfx((ctx, d) => {
        noise(ctx, d, { dur: 1.2, vol: 0.4, filter: 600, slideFilter: 80 });
        tone(ctx, d, { type: 'sawtooth', freq: 90, slide: 30, dur: 1, vol: 0.3, filter: 400 });
      }, { pos: a.entityPos(e.target) });
      const towerText = ANNOUNCEMENTS['tower_destroyed']?.();
      if (towerText) speak(towerText, a.getVolume('voice'));
      break;

    case 'objectiveTaken':
      a.sfx((ctx, d) => {
        tone(ctx, d, { type: 'sawtooth', freq: 392, dur: 0.2, vol: 0.2, filter: 2000 });
        tone(ctx, d, { freq: 523, dur: 0.2, vol: 0.2, delay: 0.15, filter: 2000 });
        tone(ctx, d, { freq: 659, dur: 0.35, vol: 0.22, delay: 0.3, filter: 2000 });
      }, { channel: 'sfx' });
      break;

    case 'announcement': {
      const text = typeof e.params?.text === 'string' ? e.params.text : ANNOUNCEMENTS[e.key]?.(e.params);
      a.sfx((ctx, d) => {
        tone(ctx, d, { type: 'triangle', freq: 660, dur: 0.12, vol: 0.2 });
        tone(ctx, d, { type: 'triangle', freq: 880, dur: 0.2, vol: 0.2, delay: 0.12 });
      }, { channel: 'voice' });
      if (text) speak(text, a.getVolume('voice'));
      break;
    }

    case 'ping':
      a.sfx((ctx, d) => { tone(ctx, d, { type: 'sine', freq: 1200, dur: 0.1, vol: 0.25 }); tone(ctx, d, { freq: 1200, dur: 0.15, vol: 0.2, delay: 0.12 }); }, { pos: e.pos, channel: 'ui' });
      break;

    default:
      break;
  }
}

export type { SfxFn };
