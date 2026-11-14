/**
 * Icônes générées en SVG et portraits illustrés locaux. Propriétaire : mistral-ui.
 * Portraits de secours procéduraux (palette + emblème de classe + motif déterministe),
 * icônes de sorts, d'objets, de sorts d'invocateur et de rôles.
 */
import { CHAMPIONS, ITEMS, type AbilityDef, type ItemDef, type Role, type SummonerId } from '@lov/shared';
import type { JSX } from 'preact';
import { hashString } from '../logic/format.ts';
import { ChampionArtwork } from './ChampionArtwork.tsx';

type SvgProps = { size?: number | string; class?: string; title?: string };

// ------------------------------------------------------------------ emblèmes de classe (viewBox 24)
const EMBLEM: Record<string, string> = {
  tank: 'M12 2 L20 5 V11 C20 16.5 16.5 20.5 12 22 C7.5 20.5 4 16.5 4 11 V5 Z',
  fighter: 'M12 1.5 L14.2 4 V14.5 H17.5 V16.6 H13.3 V21 L12 22.5 L10.7 21 V16.6 H6.5 V14.5 H9.8 V4 Z',
  mage: 'M12 1.5 L14.6 9.4 L22.5 12 L14.6 14.6 L12 22.5 L9.4 14.6 L1.5 12 L9.4 9.4 Z',
  marksman: 'M4 3 C14 5 18 9 19.5 12 C18 15 14 19 4 21 C9 17 11 14.5 11 12 C11 9.5 9 7 4 3 Z M8 11.2 H22 V12.8 H8 Z',
  assassin: 'M15 2 A10 10 0 1 0 15 22 A12 12 0 0 1 15 2 Z M17 6 L19 11 L22 12 L19 13 L17 18 L18.2 12 Z',
  support: 'M12 6.5 A5.5 5.5 0 1 1 11.99 6.5 Z M11.2 0.5 H12.8 V4.5 H11.2 Z M11.2 19.5 H12.8 V23.5 H11.2 Z M0.5 11.2 H4.5 V12.8 H0.5 Z M19.5 11.2 H23.5 V12.8 H19.5 Z M3.4 4.5 L4.5 3.4 L7.3 6.2 L6.2 7.3 Z M16.7 17.8 L17.8 16.7 L20.6 19.5 L19.5 20.6 Z M19.5 3.4 L20.6 4.5 L17.8 7.3 L16.7 6.2 Z M6.2 16.7 L7.3 17.8 L4.5 20.6 L3.4 19.5 Z',
};

export function ClassEmblem({ cls, size = 16, color = 'currentColor' }: { cls: string; size?: number; color?: string }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true">
      <path d={EMBLEM[cls] ?? EMBLEM.mage} fill={color} fill-rule="evenodd" />
    </svg>
  );
}

// ------------------------------------------------------------------ portrait de champion
/**
 * Illustration locale avec portrait procédural de secours si l'image manque ou échoue.
 * Le cadre SVG conserve le recadrage, les états et l'accessibilité de toutes les vues existantes.
 * `variant="splash"` : format portrait 3:4 pour les cartes de chargement / draft.
 */
export function ChampionPortrait({ id, size = 64, variant = 'square', class: cls, grayscale }: { id: string; size?: number | string; variant?: 'square' | 'splash' | 'round'; class?: string; grayscale?: boolean }) {
  const c = CHAMPIONS[id];
  const pal = c?.palette ?? { primary: '#777', secondary: '#222', accent: '#ddd' };
  const h = hashString(id);
  const uid = `lp-${id}-${variant}`;
  const W = 100;
  const H = variant === 'splash' ? 133 : 100;
  const cx = 50;
  const cy = variant === 'splash' ? 58 : 50;
  const rays = 7 + (h % 6);
  const rot = (h >> 4) % 360;
  const ringR = 26 + ((h >> 9) % 10);
  const letter = (c?.name ?? id).charAt(0).toUpperCase();
  const rayPaths: JSX.Element[] = [];
  for (let i = 0; i < rays; i++) {
    const a0 = ((rot + (i * 360) / rays) * Math.PI) / 180;
    const a1 = a0 + Math.PI / rays / 1.6;
    const R = 120;
    rayPaths.push(<path key={i} d={`M${cx} ${cy} L${cx + Math.cos(a0) * R} ${cy + Math.sin(a0) * R} L${cx + Math.cos(a1) * R} ${cy + Math.sin(a1) * R} Z`} />);
  }
  const shards: JSX.Element[] = [];
  for (let i = 0; i < 5; i++) {
    const hh = hashString(`${id}${i}`);
    const x = hh % W;
    const y = (hh >> 8) % H;
    const s = 3 + ((hh >> 16) % 6);
    shards.push(<path key={i} d={`M${x} ${y - s} L${x + s * 0.6} ${y} L${x} ${y + s} L${x - s * 0.6} ${y} Z`} opacity={0.35 + ((hh >> 20) % 40) / 100} />);
  }
  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      width={size}
      height={variant === 'splash' && typeof size === 'number' ? size * 1.33 : size}
      class={`lov-portrait ${variant} ${grayscale ? 'gray' : ''} ${cls ?? ''}`}
      role="img"
      aria-label={c?.name ?? id}
      preserveAspectRatio="xMidYMid slice"
    >
      <defs>
        <radialGradient id={`${uid}-bg`} cx="50%" cy={variant === 'splash' ? '42%' : '45%'} r="75%">
          <stop offset="0%" stop-color={pal.primary} />
          <stop offset="55%" stop-color={pal.secondary} />
          <stop offset="100%" stop-color="#05060c" />
        </radialGradient>
        <linearGradient id={`${uid}-glyph`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#fff" />
          <stop offset="45%" stop-color={pal.accent} />
          <stop offset="100%" stop-color={pal.primary} />
        </linearGradient>
        <radialGradient id={`${uid}-vig`} cx="50%" cy="50%" r="70%">
          <stop offset="60%" stop-color="#000" stop-opacity="0" />
          <stop offset="100%" stop-color="#000" stop-opacity="0.65" />
        </radialGradient>
      </defs>
      <rect width={W} height={H} fill={`url(#${uid}-bg)`} />
      <g fill={pal.accent} opacity="0.09">{rayPaths}</g>
      <circle cx={cx} cy={cy} r={ringR} fill="none" stroke={pal.accent} stroke-opacity="0.35" stroke-width="1.2" />
      <circle cx={cx} cy={cy} r={ringR + 6} fill="none" stroke={pal.primary} stroke-opacity="0.4" stroke-width="0.6" stroke-dasharray="2 3" />
      <g transform={`translate(${cx - 34} ${cy - 34}) scale(2.83)`} opacity="0.22">
        <path d={EMBLEM[c?.class ?? 'mage']} fill={pal.accent} fill-rule="evenodd" />
      </g>
      <g fill={pal.accent}>{shards}</g>
      <text
        x={cx}
        y={cy + 15}
        text-anchor="middle"
        font-family="Cinzel, 'Times New Roman', serif"
        font-weight="800"
        font-size="44"
        fill={`url(#${uid}-glyph)`}
        stroke="#0a0a12"
        stroke-width="1.6"
        paint-order="stroke"
      >
        {letter}
      </text>
      <ChampionArtwork id={id} width={W} height={H} />
      <rect width={W} height={H} fill={`url(#${uid}-vig)`} />
    </svg>
  );
}

// ------------------------------------------------------------------ icônes de sorts
const TARGET_GLYPH: Record<string, string> = {
  skillshot: 'M3 13 L15 13 L15 9 L22 12 L15 15 L15 11 L3 11 Z M3 8 L9 8 L9 9.4 L3 9.4 Z M3 14.6 L9 14.6 L9 16 L3 16 Z',
  targeted: 'M11 2 H13 V7 H11 Z M11 17 H13 V22 H11 Z M2 11 H7 V13 H2 Z M17 11 H22 V13 H17 Z M12 8.5 A3.5 3.5 0 1 1 11.99 8.5 Z',
  ground: 'M12 4 A8 8 0 1 1 11.99 4 Z M12 6.5 A5.5 5.5 0 1 0 12.01 6.5 Z M12 9.5 A2.5 2.5 0 1 1 11.99 9.5 Z',
  cone: 'M3 12 L21 3 C23 9 23 15 21 21 Z M8 12 L18 7.5 C19 10.5 19 13.5 18 16.5 Z',
  self: 'M12 3 A9 9 0 1 1 11.99 3 Z M12 5 A7 7 0 1 0 12.01 5 Z M12 8 A4 4 0 1 1 11.99 8 Z',
  dash: 'M2 5 L9 12 L2 19 L5 19 L12 12 L5 5 Z M10 5 L17 12 L10 19 L13 19 L20 12 L13 5 Z',
  targetedDash: 'M2 6 L8 12 L2 18 L4.5 18 L10.5 12 L4.5 6 Z M9 6 L15 12 L9 18 L11.5 18 L17.5 12 L11.5 6 Z M20 9.5 A2.5 2.5 0 1 1 19.99 9.5 Z',
  global: 'M12 2 A10 10 0 1 1 11.99 2 Z M12 4 A8 8 0 1 0 12.01 4 Z M11 3 H13 V21 H11 Z M3 11 H21 V13 H3 Z M12 4 C7 8 7 16 12 20 C9 16 9 8 12 4 Z M12 4 C17 8 17 16 12 20 C15 16 15 8 12 4 Z',
  passive: 'M12 2 L20.7 7 V17 L12 22 L3.3 17 V7 Z M12 5 L18 8.5 V15.5 L12 19 L6 15.5 V8.5 Z M12 8 L15.5 10 V14 L12 16 L8.5 14 V10 Z',
};

const SLOT_HUE: Record<string, number> = { P: 0, Q: 0, W: 35, E: -35, R: 70 };

export function AbilityIcon({ ability, championId, size = 48 }: { ability: AbilityDef; championId: string; size?: number | string }) {
  const pal = CHAMPIONS[championId]?.palette ?? { primary: '#888', secondary: '#222', accent: '#eee' };
  const uid = `la-${ability.id}`;
  const glyph = ability.slot === 'P' ? TARGET_GLYPH.passive : (TARGET_GLYPH[ability.targeting] ?? TARGET_GLYPH.self);
  const hue = SLOT_HUE[ability.slot] ?? 0;
  const dmg = ability.damage?.type;
  const tint = dmg === 'physical' ? '#ffb36b' : dmg === 'magic' ? '#c79bff' : dmg === 'true' ? '#ffffff' : pal.accent;
  const h = hashString(ability.id);
  return (
    <svg viewBox="0 0 48 48" width={size} height={size} class="lov-ability-icon" role="img" aria-label={ability.name}>
      <defs>
        <linearGradient id={`${uid}-bg`} x1="0" y1="0" x2="1" y2="1" gradientTransform={`rotate(${h % 90})`}>
          <stop offset="0%" stop-color={pal.primary} style={{ filter: `hue-rotate(${hue}deg)` }} />
          <stop offset="100%" stop-color={pal.secondary} />
        </linearGradient>
        <radialGradient id={`${uid}-glow`} cx="50%" cy="50%" r="50%">
          <stop offset="0%" stop-color={tint} stop-opacity="0.55" />
          <stop offset="100%" stop-color={tint} stop-opacity="0" />
        </radialGradient>
      </defs>
      <rect width="48" height="48" fill={`url(#${uid}-bg)`} />
      <g opacity="0.18" stroke={pal.accent} stroke-width="0.8" fill="none">
        <circle cx="24" cy="24" r={10 + (h % 8)} />
        <circle cx="24" cy="24" r={18 + ((h >> 3) % 6)} stroke-dasharray="3 4" />
      </g>
      <circle cx="24" cy="24" r="18" fill={`url(#${uid}-glow)`} />
      <g transform="translate(8 8) scale(1.333)">
        <path d={glyph} fill={pal.accent} fill-rule="evenodd" stroke="#08080e" stroke-width="0.6" paint-order="stroke" />
      </g>
      {ability.slot === 'R' && <path d="M0 0 H14 L0 14 Z M48 48 H34 L48 34 Z" fill={tint} opacity="0.7" />}
    </svg>
  );
}

// ------------------------------------------------------------------ objets
const ITEM_GLYPH: Record<string, string> = {
  sword: 'M18 2 L22 2 L22 6 L10 18 L12 20 L10.5 21.5 L8 19 L5 22 L2 19 L5 16 L2.5 13.5 L4 12 L6 14 Z',
  blade: 'M20 2 C21 8 16 15 9 18 L11 20 L9.5 21.5 L7 19 L4 22 L2 20 L5 17 L2.5 14.5 L4 13 L6 15 C9 8 14 4 20 2 Z',
  tome: 'M4 3 H17 C18.7 3 20 4.3 20 6 V21 H6.5 C5.1 21 4 19.9 4 18.5 Z M6.5 17.5 C5.9 17.5 5.5 18 5.5 18.5 C5.5 19 5.9 19.5 6.5 19.5 H18.5 V17.5 Z M9 7 H16 V8.5 H9 Z M9 10 H14 V11.5 H9 Z',
  gem: 'M7 3 H17 L22 9 L12 22 L2 9 Z M8 4.5 L4.6 8.8 H9.4 Z M16 4.5 L14.6 8.8 H19.4 Z M10 4.5 L12 8.5 L14 4.5 Z M5 10.3 L11 18.5 L9.6 10.3 Z M19 10.3 L14.4 10.3 L13 18.5 Z',
  boots: 'M6 2 H13 V12 L20 15 C21.5 15.7 22 17 22 18.5 V21 H4 V16 L6 13 Z M4 19 H22 V21 H4 Z',
  potion: 'M9 2 H15 V4 H14 V8 C17.5 9.3 20 12.4 20 16 C20 19.9 16.4 23 12 23 C7.6 23 4 19.9 4 16 C4 12.4 6.5 9.3 10 8 V4 H9 Z M6 15.5 C6.4 19 9 21 12 21 C15 21 17.6 19 18 15.5 Z',
  ring: 'M12 7 A8 8 0 1 1 11.99 7 Z M12 10 A5 5 0 1 0 12.01 10 Z M9 2 H15 L16.5 5 L12 8.5 L7.5 5 Z',
  shield: 'M12 2 L21 5.5 V11 C21 16.5 17 20.8 12 22.5 C7 20.8 3 16.5 3 11 V5.5 Z M12 5 V19.5 C15.5 18 18.5 15 18.5 11 V7.2 Z',
  eye: 'M12 5 C17 5 21 9 23 12 C21 15 17 19 12 19 C7 19 3 15 1 12 C3 9 7 5 12 5 Z M12 8 A4 4 0 1 0 12.01 8 Z M12 10.3 A1.7 1.7 0 1 1 11.99 10.3 Z',
  dagger: 'M12 1 L14.5 6 V14 H17 V16 H13.2 V19 L14 20 L12 23 L10 20 L10.8 19 V16 H7 V14 H9.5 V6 Z',
  cloak: 'M8 2 H16 L17 5 C20 9 21.5 15 22 22 H15 L12 16 L9 22 H2 C2.5 15 4 9 7 5 Z M10 3.5 L12 7 L14 3.5 Z',
  wand: 'M3 20 L15 8 L16.5 9.5 L4.5 21.5 Z M18 2 L19.2 4.8 L22 6 L19.2 7.2 L18 10 L16.8 7.2 L14 6 L16.8 4.8 Z',
  hammer: 'M4 4 H15 L18 7 V11 H15 L13.5 9.5 H12 V22 H9 V9.5 H4 Z',
  hourglass: 'M5 2 H19 V4 H17.5 C17.5 8 14.5 10.5 13 12 C14.5 13.5 17.5 16 17.5 20 H19 V22 H5 V20 H6.5 C6.5 16 9.5 13.5 11 12 C9.5 10.5 6.5 8 6.5 4 H5 Z M8.6 20 H15.4 C15 17.4 13 15.5 12 14.6 C11 15.5 9 17.4 8.6 20 Z',
  chest: 'M3 4 L8 2 C9 4 10.5 5 12 5 C13.5 5 15 4 16 2 L21 4 L19.5 10 L18 9.5 V22 H6 V9.5 L4.5 10 Z M9 11 H15 V12.5 H9 Z M9 14.5 H15 V16 H9 Z',
  orb: 'M12 3 A9 9 0 1 1 11.99 3 Z M9 7.5 A3 3 0 1 0 9.01 7.5 Z',
  heart: 'M12 21 C5 16 2 12.5 2 8.5 C2 5.5 4.5 3 7.5 3 C9.5 3 11 4 12 5.5 C13 4 14.5 3 16.5 3 C19.5 3 22 5.5 22 8.5 C22 12.5 19 16 12 21 Z',
  claw: 'M5 21 C5 13 7 6 12 2 C10 7 10 12 11 15 C12 10 14 6 18 3 C16 8 15.5 13 16 17 C17 14 19 12 22 11 C19 15 18 19 17 22 Z',
  crown: 'M2 7 L7 11 L12 3 L17 11 L22 7 L20 19 H4 Z M4 20.5 H20 V22 H4 Z',
  axe: 'M11 2 H13 V22 H11 Z M13 3 C18 3 21 6 21 10 C21 13 19 15 17 15 C17 12 15.5 10 13 10 Z M11 5 C8 5 6 6.5 6 8.5 C7.5 8 9.5 8 11 9 Z',
  censer: 'M11 1 H13 V4 H11 Z M12 4 L16 7 H8 Z M6 8 H18 C18 13 15.5 16 12 16 C8.5 16 6 13 6 8 Z M9 17 H15 L16 20 H8 Z M7 21 H17 V23 H7 Z',
  bow: 'M5 2 C17 4 20 10 20 12 C20 14 17 20 5 22 L6.5 20.2 C14 18.5 17.5 15 17.5 12 C17.5 9 14 5.5 6.5 3.8 Z M4 11 H19 V13 H4 Z M2 9 L5 12 L2 15 Z',
};

const CATEGORY_COLOR: Record<string, [string, string]> = {
  starter: ['#5b8a5a', '#1d2b1c'],
  consumable: ['#b7434a', '#2a0f13'],
  boots: ['#8a6a46', '#2a1c10'],
  basic: ['#7f8ea3', '#1a1f29'],
  epic: ['#8b5bd6', '#1d1233'],
  legendary: ['#e2b64a', '#3a2408'],
  trinket: ['#d7c24a', '#2c2709'],
};

function itemGlyphKey(item: ItemDef): string {
  const k = `${item.icon} ${item.id}`.toLowerCase();
  const table: [RegExp, string][] = [
    [/hourglass|sablier|stasis/, 'hourglass'],
    [/crown|couronne/, 'crown'],
    [/executioner|bourreau|axe|hache/, 'axe'],
    [/censer|encensoir|lantern/, 'censer'],
    [/veil|voile|negatron/, 'cloak'],
    [/rafale|gust/, 'blade'],
    [/colossus|colosse_masse|masse/, 'hammer'],
    [/cloth|rockplate/, 'chest'],
    [/potion|flask|elixir|biscuit/, 'potion'],
    [/ward|eye|trinket|totem/, 'eye'],
    [/boot|greave|tabi|shoe/, 'boots'],
    [/machete|claw|griffe/, 'claw'],
    [/bow|arc/, 'bow'],
    [/dagger|dague/, 'dagger'],
    [/blade|lame|edge|doran_blade/, 'blade'],
    [/sword|epee|saber/, 'sword'],
    [/tome|book|codex|grimoire/, 'tome'],
    [/ruby|crystal|gem|sapphire|saphir|jewel/, 'gem'],
    [/ring|anneau/, 'ring'],
    [/shield|bouclier|aegis|egide/, 'shield'],
    [/cloak|cape|mantle|negation/, 'cloak'],
    [/wand|rod|staff|baton|baguette|sceptre/, 'wand'],
    [/hammer|marteau|mace/, 'hammer'],
    [/armor|vest|plate|cuirasse|tissu|chest/, 'chest'],
    [/orb|pearl|perle/, 'orb'],
    [/heart|coeur|warmog/, 'heart'],
  ];
  for (const [re, g] of table) if (re.test(k)) return g;
  const s = item.stats;
  if (s.critChance) return 'cloak';
  if (s.attackSpeed) return 'dagger';
  if (s.ad || s.armorPen) return 'sword';
  if (s.ap || s.magicPen) return 'wand';
  if (s.armor) return 'chest';
  if (s.mr) return 'cloak';
  if (s.maxHp) return 'heart';
  if (s.maxMana) return 'orb';
  return 'gem';
}

function itemTint(item: ItemDef): string {
  const s = item.stats;
  if (s.ap || s.magicPen) return '#c48bff';
  if (s.critChance || s.attackSpeed) return '#ffd16b';
  if (s.ad || s.armorPen || s.lifesteal) return '#ff9a5c';
  if (s.armor) return '#e8d08c';
  if (s.mr) return '#7fc6ff';
  if (s.maxHp || s.hpRegen) return '#ff6d7a';
  if (s.maxMana || s.manaRegen) return '#6fa8ff';
  if (s.moveSpeed) return '#9ff0c2';
  return '#f2f2f2';
}

export function ItemIcon({ itemId, size = 40, class: cls }: { itemId: string | null | undefined; size?: number | string; class?: string }) {
  const item = itemId ? ITEMS[itemId] : undefined;
  if (!item) return <span class={`lov-item-empty ${cls ?? ''}`} style={{ width: size, height: size }} />;
  const [c1, c2] = CATEGORY_COLOR[item.category] ?? CATEGORY_COLOR.basic!;
  const uid = `li-${item.category}`;
  const tint = itemTint(item);
  return (
    <svg viewBox="0 0 40 40" width={size} height={size} class={`lov-item-icon cat-${item.category} ${cls ?? ''}`} role="img" aria-label={item.name}>
      <defs>
        <radialGradient id={`${uid}-bg`} cx="35%" cy="30%" r="85%">
          <stop offset="0%" stop-color={c1} />
          <stop offset="100%" stop-color={c2} />
        </radialGradient>
      </defs>
      <rect width="40" height="40" fill={`url(#${uid}-bg)`} />
      <g transform="translate(6 6) scale(1.1667)">
        <path d={ITEM_GLYPH[itemGlyphKey(item)]} fill={tint} fill-rule="evenodd" stroke="#0a0a10" stroke-width="0.9" paint-order="stroke" />
      </g>
      <rect x="0.75" y="0.75" width="38.5" height="38.5" fill="none" stroke={c1} stroke-opacity="0.9" stroke-width="1.5" />
    </svg>
  );
}

// ------------------------------------------------------------------ sorts d'invocateur
const SUMMONER_ART: Record<SummonerId, { glyph: string; c1: string; c2: string }> = {
  flash: { glyph: 'M13 1 L4 14 H11 L9 23 L20 9 H13 Z', c1: '#ffe680', c2: '#7a5a00' },
  ignite: { glyph: 'M12 1 C14 6 19 8 19 14.5 C19 19 15.9 22.5 12 22.5 C8.1 22.5 5 19 5 14.5 C5 11 7 9 8.5 7.5 C8.5 10 9.5 11.5 11 12 C10 8 11 4 12 1 Z M12 14 C13.5 16 15 17 15 18.8 C15 20.4 13.7 21.3 12 21.3 C10.3 21.3 9 20.4 9 18.8 C9 17 10.5 16 12 14 Z', c1: '#ff8a3c', c2: '#5c1300' },
  heal: { glyph: 'M9 2 H15 V9 H22 V15 H15 V22 H9 V15 H2 V9 H9 Z', c1: '#7dffa0', c2: '#0b4a1f' },
  barrier: { glyph: 'M12 1.5 L21 5 V11 C21 16.5 17 21 12 22.5 C7 21 3 16.5 3 11 V5 Z M12 4.5 L6 6.8 V11 C6 14.8 8.6 18.2 12 19.5 C15.4 18.2 18 14.8 18 11 V6.8 Z', c1: '#ffe9a8', c2: '#6b4a0a' },
  exhaust: { glyph: 'M12 2 C17.5 2 22 6.5 22 12 C22 15.5 19 18 16 18 C13 18 11 16 11 13.5 C11 11.5 12.5 10 14.3 10 C15.7 10 16.8 11 16.8 12.3 C16.8 13.3 16 14 15 14 C15.8 15.5 19 15 19 12 C19 8.1 15.9 5 12 5 C7.6 5 4 8.6 4 13 C4 17.4 7.6 21 12 21 V23 C5.9 23 1 18.1 1 12 C1 6.5 5.9 2 12 2 Z', c1: '#d8b48a', c2: '#3b2410' },
  teleport: { glyph: 'M12 2 A10 10 0 1 1 11.99 2 Z M12 5 A7 7 0 1 0 12.01 5 Z M12 8 C14.2 8 16 9.8 16 12 C16 14.2 14.2 16 12 16 C9.8 16 8 14.2 8 12 C8 9.8 9.8 8 12 8 Z', c1: '#d18bff', c2: '#2c0b4a' },
  smite: { glyph: 'M15 1 L7 12 H12 L8 23 L18 9 H13 L17 1 Z M3 18 L6 15 L7 16 L4 19 Z M21 6 L18 9 L17 8 L20 5 Z', c1: '#ffcf4a', c2: '#5a2c00' },
  ghost: { glyph: 'M12 2 C17 2 20 6 20 11 V22 L17.3 19.5 L14.7 22 L12 19.5 L9.3 22 L6.7 19.5 L4 22 V11 C4 6 7 2 12 2 Z M9 9 A1.6 1.6 0 1 0 9.01 9 Z M15 9 A1.6 1.6 0 1 0 15.01 9 Z', c1: '#8ff4ff', c2: '#08364a' },
  cleanse: { glyph: 'M12 1 L13.8 8.2 L21 6 L15.8 11.2 L22 15 L14.6 15 L15 22.5 L12 16 L9 22.5 L9.4 15 L2 15 L8.2 11.2 L3 6 L10.2 8.2 Z', c1: '#e0f6ff', c2: '#1b3c5a' },
};

export function SummonerIcon({ id, size = 32 }: { id: SummonerId | string; size?: number | string }) {
  const art = SUMMONER_ART[id as SummonerId] ?? SUMMONER_ART.flash;
  const uid = `ls-${id}`;
  return (
    <svg viewBox="0 0 32 32" width={size} height={size} class="lov-summoner-icon" role="img" aria-label={id}>
      <defs>
        <radialGradient id={`${uid}-bg`} cx="50%" cy="40%" r="75%">
          <stop offset="0%" stop-color={art.c1} stop-opacity="0.55" />
          <stop offset="100%" stop-color={art.c2} />
        </radialGradient>
      </defs>
      <rect width="32" height="32" fill={`url(#${uid}-bg)`} />
      <g transform="translate(4 4)">
        <path d={art.glyph} fill={art.c1} fill-rule="evenodd" stroke="#08080c" stroke-width="0.8" paint-order="stroke" />
      </g>
    </svg>
  );
}

// ------------------------------------------------------------------ rôles
const ROLE_GLYPH: Record<Role, string> = {
  top: 'M3 3 H17 L13 7 H7 V13 L3 17 Z M9 9 H21 V21 H9 Z M12 12 V18 H18 V12 Z',
  jungle: 'M12 2 C16 6 18 10 18 14 C18 18 15.3 21 12 22 C8.7 21 6 18 6 14 C6 10 8 6 12 2 Z M11.2 9 H12.8 V20 H11.2 Z M12 14 L15.5 11 L16.4 12.1 L12.8 15.2 Z',
  mid: 'M3 18 L18 3 L21 3 L21 6 L6 21 L3 21 Z M3 3 H10 L3 10 Z M21 21 H14 L21 14 Z',
  bot: 'M7 3 L21 3 L21 17 L17 21 L17 7 L3 7 Z M3 11 H15 V21 H3 Z M6 14 V18 H12 V14 Z',
};

export function RoleIcon({ role, size = 18 }: { role: Role; size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} class="lov-role-icon" aria-hidden="true">
      <path d={ROLE_GLYPH[role]} fill="currentColor" fill-rule="evenodd" />
    </svg>
  );
}

// ------------------------------------------------------------------ glyphes divers
const MISC: Record<string, string> = {
  gold: 'M12 3 C17 3 21 4.8 21 7 V17 C21 19.2 17 21 12 21 C7 21 3 19.2 3 17 V7 C3 4.8 7 3 12 3 Z M12 5 C8 5 5 6 5 7 C5 8 8 9 12 9 C16 9 19 8 19 7 C19 6 16 5 12 5 Z',
  tower: 'M7 2 H9 V4 H11 V2 H13 V4 H15 V2 H17 V7 L15 9 V19 L18 22 H6 L9 19 V9 L7 7 Z',
  sword: ITEM_GLYPH.sword!,
  skull: 'M12 2 C17 2 21 5.8 21 10.5 C21 13.4 19.5 15.5 17.5 16.8 V20 C17.5 21.1 16.6 22 15.5 22 H8.5 C7.4 22 6.5 21.1 6.5 20 V16.8 C4.5 15.5 3 13.4 3 10.5 C3 5.8 7 2 12 2 Z M8.5 9 A2.2 2.2 0 1 0 8.51 9 Z M15.5 9 A2.2 2.2 0 1 0 15.51 9 Z M11 14 L12 12 L13 14 Z',
  crown: 'M2 7 L7 11 L12 3 L17 11 L22 7 L20 19 H4 Z M4 20.5 H20 V22 H4 Z',
  gear: 'M10.3 1 H13.7 L14.3 4.1 L16.6 5.1 L19.2 3.3 L20.7 4.8 L18.9 7.4 L19.9 9.7 L23 10.3 V13.7 L19.9 14.3 L18.9 16.6 L20.7 19.2 L19.2 20.7 L16.6 18.9 L14.3 19.9 L13.7 23 H10.3 L9.7 19.9 L7.4 18.9 L4.8 20.7 L3.3 19.2 L5.1 16.6 L4.1 14.3 L1 13.7 V10.3 L4.1 9.7 L5.1 7.4 L3.3 4.8 L4.8 3.3 L7.4 5.1 L9.7 4.1 Z M12 8 A4 4 0 1 0 12.01 8 Z',
  lock: 'M7 10 V7 C7 4.2 9.2 2 12 2 C14.8 2 17 4.2 17 7 V10 H19 V22 H5 V10 Z M9.5 10 H14.5 V7 C14.5 5.6 13.4 4.5 12 4.5 C10.6 4.5 9.5 5.6 9.5 7 Z',
  minion: 'M12 3 L19 9 L16 21 H8 L5 9 Z M9.5 10 A1.5 1.5 0 1 0 9.51 10 Z M14.5 10 A1.5 1.5 0 1 0 14.51 10 Z',
  dragon: 'M3 20 C5 14 8 10 12 9 L10 4 L14 7 L17 3 L17.5 8 C20.5 9.5 22 12.5 21 16 L18 13.5 L17 18 L14 15 L12 19 L10 15.5 Z',
  leviathan: 'M2 16 C5 9 11 6 17 7 C14 8 12 10 12 12 C15 10 19 10 22 12 C19 13 17 15 17 18 C14 15 9 15 6 19 Z',
  grub: 'M5 14 C5 10 8 7 12 7 C16 7 19 10 19 14 C19 17 16.5 19 13.5 19 H10.5 C7.5 19 5 17 5 14 Z M9 11.5 A1.3 1.3 0 1 0 9.01 11.5 Z M15 11.5 A1.3 1.3 0 1 0 15.01 11.5 Z M8 5 L10 8 L9 8.5 Z M16 5 L14 8 L15 8.5 Z',
  herald: 'M12 2 L15 6 H19 L18 10 L21 13 L17 15 L16 21 H8 L7 15 L3 13 L6 10 L5 6 H9 Z M9.5 10 A1.6 1.6 0 1 0 9.51 10 Z M14.5 10 A1.6 1.6 0 1 0 14.51 10 Z',
  inhibitor: 'M12 2 L20 7 V17 L12 22 L4 17 V7 Z M12 7 A5 5 0 1 0 12.01 7 Z',
  nexus: 'M12 1 L16 8 L23 12 L16 16 L12 23 L8 16 L1 12 L8 8 Z',
  close: 'M5 3 L12 10 L19 3 L21 5 L14 12 L21 19 L19 21 L12 14 L5 21 L3 19 L10 12 L3 5 Z',
  copy: 'M8 2 H20 V16 H17 V5 H8 Z M4 7 H16 V22 H4 Z',
  search: 'M10 2 A8 8 0 1 1 9.99 2 Z M10 5 A5 5 0 1 0 10.01 5 Z M15.5 17.6 L17.6 15.5 L22.5 20.4 L20.4 22.5 Z',
  undo: 'M9 4 L2 10 L9 16 V12 C14 12 17.5 13.5 20 18 C19.5 11.5 15.5 8 9 8 Z',
  plus: 'M10 3 H14 V10 H21 V14 H14 V21 H10 V14 H3 V10 H10 Z',
  check: 'M2 12 L5 9 L10 14 L19 4 L22 7 L10 20 Z',
  bolt: 'M13 1 L4 14 H11 L9 23 L20 9 H13 Z',
};

export function Glyph({ name, size = 16, class: cls, title }: SvgProps & { name: keyof typeof MISC | string }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} class={`lov-glyph ${cls ?? ''}`} aria-hidden={title ? undefined : 'true'} role={title ? 'img' : undefined}>
      {title && <title>{title}</title>}
      <path d={MISC[name] ?? MISC.nexus} fill="currentColor" fill-rule="evenodd" />
    </svg>
  );
}

/** Icône d'unité non-champion (kill feed). */
export function UnitGlyph({ kind, subtype, size = 22 }: { kind: string; subtype?: string; size?: number }) {
  let name = 'minion';
  if (kind === 'tower') name = 'tower';
  else if (kind === 'inhibitor') name = 'inhibitor';
  else if (kind === 'nexus') name = 'nexus';
  else if (kind === 'monster') {
    if (subtype === 'dragon' || subtype === 'elder_dragon') name = 'dragon';
    else if (subtype === 'leviathan') name = 'leviathan';
    else if (subtype === 'grub') name = 'grub';
    else if (subtype === 'herald') name = 'herald';
    else name = 'skull';
  }
  return <Glyph name={name} size={size} />;
}
