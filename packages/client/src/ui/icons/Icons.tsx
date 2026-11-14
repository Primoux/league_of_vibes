/**
 * Icônes générées en SVG et portraits illustrés locaux. Propriétaire : mistral-ui.
 * Portraits de secours procéduraux (palette + emblème de classe + motif déterministe),
 * icônes de sorts, d'objets, de sorts d'invocateur et de rôles.
 */
import { CHAMPIONS, ITEMS, type AbilityDef, type Role, type SummonerId } from '@lov/shared';
import type { JSX } from 'preact';
import { hashString } from '../logic/format.ts';
import { ChampionArtwork, championArtworkUrl } from './ChampionArtwork.tsx';
import { abilityGlyph, CATEGORY_COLOR, ITEM_GLYPH, itemGlyphKey, itemTint, itemVariation } from './glyphs.ts';
import { getShot, studio } from '../stage/studio.ts';
import { useMini } from '../useStore.ts';

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
  useMini(studio, (s) => s.version);
  // illustration peinte (codex-champions) en priorité ; rendu 3D du modèle en secours, puis l'initiale
  const art = championArtworkUrl(id);
  const shot = art ? null : getShot(id, variant === 'splash' ? 'full' : 'bust');
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
      <g transform={`translate(${cx - 34} ${cy - 34}) scale(2.83)`} opacity={shot || art ? 0.1 : 0.22}>
        <path d={EMBLEM[c?.class ?? 'mage']} fill={pal.accent} fill-rule="evenodd" />
      </g>
      <g fill={pal.accent}>{shards}</g>
      {shot ? (
        <>
          <ellipse cx={cx} cy={H * 0.93} rx={W * 0.42} ry={H * 0.06} fill="#000" opacity="0.35" />
          <image href={shot} x="0" y={variant === 'splash' ? 4 : 2} width={W} height={H} preserveAspectRatio={variant === 'splash' ? 'xMidYMax meet' : 'xMidYMid slice'} />
        </>
      ) : (
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
      )}
      <ChampionArtwork id={id} width={W} height={H} />
      <rect width={W} height={H} fill={`url(#${uid}-vig)`} />
    </svg>
  );
}

// ------------------------------------------------------------------ icônes de sorts

export function AbilityIcon({ ability, championId, size = 48 }: { ability: AbilityDef; championId: string; size?: number | string }) {
  const pal = CHAMPIONS[championId]?.palette ?? { primary: '#888', secondary: '#222', accent: '#eee' };
  const uid = `la-${ability.id}`;
  const glyph = abilityGlyph(ability);
  const dmg = ability.damage?.type;
  const tint = dmg === 'physical' ? '#ffb36b' : dmg === 'magic' ? '#d2a8ff' : dmg === 'true' ? '#ffffff' : pal.accent;
  const h = hashString(ability.id);
  const ang = (h % 360) * (Math.PI / 180);
  const rays = [];
  for (let i = 0; i < 6; i++) {
    const a = ang + (i * Math.PI) / 3;
    rays.push(<path key={i} d={`M24 24 L${24 + Math.cos(a) * 40} ${24 + Math.sin(a) * 40} L${24 + Math.cos(a + 0.22) * 40} ${24 + Math.sin(a + 0.22) * 40} Z`} />);
  }
  return (
    <svg viewBox="0 0 48 48" width={size} height={size} class="lov-ability-icon" role="img" aria-label={ability.name}>
      <defs>
        <radialGradient id={`${uid}-bg`} cx="30%" cy="25%" r="95%">
          <stop offset="0%" stop-color={pal.primary} />
          <stop offset="55%" stop-color={pal.secondary} />
          <stop offset="100%" stop-color="#05060c" />
        </radialGradient>
        <radialGradient id={`${uid}-glow`} cx="50%" cy="50%" r="50%">
          <stop offset="0%" stop-color={tint} stop-opacity="0.7" />
          <stop offset="60%" stop-color={tint} stop-opacity="0.12" />
          <stop offset="100%" stop-color={tint} stop-opacity="0" />
        </radialGradient>
        <linearGradient id={`${uid}-glyph`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#ffffff" />
          <stop offset="55%" stop-color={pal.accent} />
          <stop offset="100%" stop-color={tint} />
        </linearGradient>
        <filter id={`${uid}-f`} x="-30%" y="-30%" width="160%" height="160%">
          <feGaussianBlur stdDeviation="1.6" result="b" />
          <feMerge>
            <feMergeNode in="b" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
      <rect width="48" height="48" fill={`url(#${uid}-bg)`} />
      <g fill={pal.accent} opacity="0.1">{rays}</g>
      <circle cx="24" cy="24" r="20" fill={`url(#${uid}-glow)`} />
      <circle cx="24" cy="24" r={13 + (h % 5)} fill="none" stroke={pal.accent} stroke-opacity="0.22" stroke-width="0.7" stroke-dasharray="2 3" />
      <g transform="translate(7 7) scale(1.4167)" filter={`url(#${uid}-f)`}>
        <path d={glyph} fill={`url(#${uid}-glyph)`} fill-rule="evenodd" stroke="#05060c" stroke-width="0.7" paint-order="stroke" />
      </g>
      <path d="M0 0 H48 V16 C32 10 16 10 0 16 Z" fill="#fff" opacity="0.08" />
      {ability.slot === 'R' && <path d="M0 0 H12 L0 12 Z M48 48 H36 L48 36 Z M48 0 V12 L36 0 Z M0 48 V36 L12 48 Z" fill={tint} opacity="0.75" />}
    </svg>
  );
}

// ------------------------------------------------------------------ objets
export function ItemIcon({ itemId, size = 40, class: cls }: { itemId: string | null | undefined; size?: number | string; class?: string }) {
  const item = itemId ? ITEMS[itemId] : undefined;
  if (!item) return <span class={`lov-item-empty ${cls ?? ''}`} style={{ width: size, height: size }} />;
  const [c1, c2] = CATEGORY_COLOR[item.category] ?? CATEGORY_COLOR.basic!;
  const uid = `li-${item.id}`;
  const tint = itemTint(item);
  const v = itemVariation(item);
  return (
    <svg viewBox="0 0 40 40" width={size} height={size} class={`lov-item-icon cat-${item.category} ${cls ?? ''}`} role="img" aria-label={item.name}>
      <defs>
        <radialGradient id={`${uid}-bg`} cx={`${v.lightX}%`} cy="28%" r="90%">
          <stop offset="0%" stop-color={c1} style={{ filter: `hue-rotate(${v.hue}deg)` }} />
          <stop offset="100%" stop-color={c2} />
        </radialGradient>
        <radialGradient id={`${uid}-glow`} cx="50%" cy="50%" r="50%">
          <stop offset="0%" stop-color={tint} stop-opacity="0.45" />
          <stop offset="100%" stop-color={tint} stop-opacity="0" />
        </radialGradient>
      </defs>
      <rect width="40" height="40" fill={`url(#${uid}-bg)`} />
      <g opacity="0.14" stroke={tint} stroke-width="0.6" fill="none">
        <circle cx="20" cy="20" r={9 + v.ring} />
        <circle cx="20" cy="20" r={15 + v.ring / 2} stroke-dasharray="2 3" />
      </g>
      <circle cx="20" cy="20" r="15" fill={`url(#${uid}-glow)`} />
      <g transform={`translate(6 6) scale(1.1667) rotate(${v.tilt} 12 12)`}>
        <path d={ITEM_GLYPH[itemGlyphKey(item)]} fill={tint} fill-rule="evenodd" stroke="#0a0a10" stroke-width="0.9" paint-order="stroke" />
      </g>
      {item.category === 'legendary' && <path d={`M${v.gemX} 4.5 L${v.gemX + 2.5} 7 L${v.gemX} 9.5 L${v.gemX - 2.5} 7 Z`} fill={v.gem} stroke="#0a0a10" stroke-width="0.6" />}
      <path d="M0 0 H40 V12 C27 8 13 8 0 12 Z" fill="#fff" opacity="0.07" />
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
