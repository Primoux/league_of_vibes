/** Formatage pur (testé). Propriétaire : mistral-ui. */
import { BUFFS, type DragonElement, type MonsterType, type Role, type Stats } from '@lov/shared';

/** 125.4 -> "2:05" ; 3725 -> "1:02:05". */
export function formatClock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** Temps de recharge affiché : "12", "4", "0.8" sous 1 s. */
export function formatCooldown(seconds: number): string {
  if (seconds <= 0) return '';
  if (seconds < 1) return seconds.toFixed(1);
  return String(Math.ceil(seconds));
}

/** 1234 -> "1,2k" ; 999 -> "999". */
export function formatGoldShort(g: number): string {
  if (Math.abs(g) < 1000) return String(Math.round(g));
  return `${(g / 1000).toFixed(1).replace('.', ',')}k`;
}

export function formatInt(n: number): string {
  return Math.round(n).toLocaleString('fr-FR');
}

export function kdaRatio(k: number, d: number, a: number): number {
  return d === 0 ? k + a : (k + a) / d;
}

export function formatKdaRatio(k: number, d: number, a: number): string {
  if (d === 0) return k + a > 0 ? 'Parfait' : '0,00';
  return kdaRatio(k, d, a).toFixed(2).replace('.', ',');
}

export const ROLE_LABEL: Record<Role, string> = { top: 'Haut', jungle: 'Jungle', mid: 'Milieu', bot: 'Bas' };

export const CLASS_LABEL: Record<string, string> = {
  tank: 'Tank',
  fighter: 'Combattant',
  mage: 'Mage',
  marksman: 'Tireur',
  assassin: 'Assassin',
  support: 'Soutien',
};

export const DRAGON_LABEL: Record<DragonElement, string> = {
  infernal: 'Drake infernal',
  ocean: 'Drake océanique',
  mountain: 'Drake des montagnes',
  cloud: 'Drake des nuages',
  hextech: 'Drake électrique',
  chemtech: 'Drake toxique',
};

export const DRAGON_COLOR: Record<DragonElement, string> = {
  infernal: '#ff6a2b',
  ocean: '#3fc4e8',
  mountain: '#c79a5b',
  cloud: '#d6e6ff',
  hextech: '#5ef0d0',
  chemtech: '#9fe03a',
};

export const MONSTER_LABEL: Partial<Record<MonsterType | 'grubs', string>> = {
  dragon: 'Drake',
  elder_dragon: 'Drake Ancien',
  grub: 'Larve',
  herald: 'Colosse du Rift',
  grubs: 'Larves',
  leviathan: 'Léviathan',
  blue_buff: 'Gardien d’azur',
  red_buff: 'Bête écarlate',
  scuttle: 'Crabe de rivière',
  gromp: 'Grenouille',
  wolves: 'Loups',
  raptors: 'Raptors',
  krugs: 'Golems de pierre',
};

export const STAT_LABEL: Record<keyof Stats, string> = {
  maxHp: 'Points de vie',
  hpRegen: 'Régén. PV (5 s)',
  maxMana: 'Mana',
  manaRegen: 'Régén. mana (5 s)',
  ad: "Dégâts d'attaque",
  ap: 'Puissance',
  armor: 'Armure',
  mr: 'Résistance magique',
  attackSpeed: "Vitesse d'attaque",
  attackRange: 'Portée',
  moveSpeed: 'Vitesse de déplacement',
  critChance: 'Chances de critique',
  abilityHaste: 'Accélération',
  lifesteal: 'Vol de vie',
  armorPen: 'Létalité',
  magicPen: 'Pénétration magique',
};

const PERCENT_STATS = new Set<keyof Stats>(['critChance', 'lifesteal']);

/** Valeur de stat lisible : "+25 %" / "+0,65" / "+40". */
export function formatStat(key: keyof Stats, value: number, signed = false): string {
  const sign = signed && value > 0 ? '+' : '';
  if (PERCENT_STATS.has(key)) return `${sign}${Math.round(value * 100)} %`;
  if (key === 'attackSpeed') return `${sign}${value.toFixed(2).replace('.', ',')}`;
  return `${sign}${Math.round(value)}`;
}

/** Valeurs par rang : "80 / 120 / 160". */
export function formatRanks(values: number[], unit = ''): string {
  const uniq = values.every((v) => v === values[0]);
  const fmt = (v: number) => `${Number.isInteger(v) ? v : v.toFixed(1).replace('.', ',')}${unit}`;
  return uniq ? fmt(values[0] ?? 0) : values.map(fmt).join(' / ');
}

/** Noms des buffs hors registre `BUFFS` (buffs de camps et d'objectifs). */
const EXTRA_BUFF_NAMES: Record<string, string> = {
  blue_buff: 'Gardien d’azur',
  red_buff: 'Bête écarlate',
  leviathan: 'Bénédiction du Léviathan',
  leviathan_buff: 'Bénédiction du Léviathan',
  elder: 'Drake Ancien',
  elder_buff: 'Drake Ancien',
  recall: 'Rappel',
};

/** Adjectif de l'âme du drake (« Âme infernale »). */
export const SOUL_LABEL: Record<DragonElement, string> = {
  infernal: 'infernale',
  ocean: 'océanique',
  mountain: 'des montagnes',
  cloud: 'des nuages',
  hextech: 'électrique',
  chemtech: 'toxique',
};

const isElement = (s: string): s is DragonElement => s in DRAGON_LABEL;

/** Nom lisible d'un buff : registre partagé `BUFFS`, puis table locale, drakes/âmes, puis id reformaté. */
export function buffName(id: string): string {
  const known = BUFFS[id]?.name ?? EXTRA_BUFF_NAMES[id];
  if (known) return known;
  const m = /^(dragon|soul)_(\w+)$/.exec(id);
  if (m && isElement(m[2]!)) return m[1] === 'dragon' ? DRAGON_LABEL[m[2]] : `Âme ${SOUL_LABEL[m[2]]}`;
  const s = id.replace(/_/g, ' ');
  return s.charAt(0).toUpperCase() + s.slice(1);
}

const DEBUFF_WORDS = /stun|root|slow|silence|knock|taunt|suppress|fear|blind|ground|bleed|burn|ignite|mark|frost|givre|poison|exhaust|grievous|anti/i;

/** Effet négatif ? (registre `BUFFS` sinon heuristique sur l'id). */
export function isDebuff(id: string): boolean {
  const def = BUFFS[id];
  if (def) return def.kind !== 'buff';
  return DEBUFF_WORDS.test(id);
}

/** Initiales affichées dans la pastille de buff. */
export function buffInitials(id: string): string {
  const name = buffName(id);
  const words = name.split(/[\s’']+/).filter((w) => w.length > 2);
  return (words.length > 1 ? words.slice(0, 2).map((w) => w[0]).join('') : name.slice(0, 2)).toUpperCase();
}

/** Hash déterministe (FNV-1a 32 bits) pour générer des motifs stables. */
export function hashString(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}
