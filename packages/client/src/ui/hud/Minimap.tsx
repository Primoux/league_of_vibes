/**
 * Minimap canvas 2D : couche statique (terrain, voies, rivière, murs, brousses, bases) pré-rendue,
 * couche dynamique redessinée à chaque snapshot (structures, champions, sbires, monstres, balises, cadre caméra).
 * Clic gauche / glisser = déplacer la caméra (uiBridge), clic droit = se déplacer, Alt+clic = ping.
 */
import { BRUSHES, CAMPS, CHAMPIONS, FOUNTAINS, INHIBITORS, LANE_PATHS, MAP_SIZE, NEXUS, PITS, TOWERS, WALLS, type Team, type Vec2 } from '@lov/shared';
import { useEffect, useRef } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { getRenderer } from '../../render/GameRenderer.ts';
import { store } from '../../state.ts';
import { entities, focusCamera, myTeam, selfEntity, worldTick } from '../game/world.ts';
import { Glyph } from '../icons/Icons.tsx';
import { formatClock, MONSTER_LABEL } from '../logic/format.ts';
import { settings } from '../settings.ts';
import { useSelect } from '../useStore.ts';

/** Objectifs épiques affichés en liste (les autres timers = camps, dessinés sur la carte). */
const EPIC: Record<string, string> = { dragon: 'dragon', elder_dragon: 'dragon', grubs: 'grub', herald: 'herald', leviathan: 'leviathan' };
const CAMP_POS = new Map(CAMPS.map((c) => [c.id, c.pos]));

const RES = 512; // résolution interne du canvas
const S = RES / MAP_SIZE;
const px = (p: Vec2): [number, number] => [p.x * S, RES - p.y * S];

const COL = {
  blue: '#4ea2ff',
  red: '#ff5a5a',
  self: '#ffd76a',
  neutral: '#e8c46a',
};

function drawStatic(): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = c.height = RES;
  const g = c.getContext('2d')!;
  // fond jungle
  const bg = g.createLinearGradient(0, RES, RES, 0);
  bg.addColorStop(0, '#16261c');
  bg.addColorStop(0.5, '#1b2a22');
  bg.addColorStop(1, '#261b1e');
  g.fillStyle = bg;
  g.fillRect(0, 0, RES, RES);
  // texture : petits points
  g.fillStyle = 'rgba(255,255,255,0.025)';
  for (let i = 0; i < 900; i++) g.fillRect((i * 97) % RES, (i * 61 + ((i * 13) % 7)) % RES, 1.5, 1.5);
  // rivière (diagonale x + y = MAP_SIZE)
  g.save();
  g.strokeStyle = 'rgba(70,150,200,0.55)';
  g.lineWidth = 46;
  g.lineCap = 'round';
  g.beginPath();
  g.moveTo(...px({ x: 600, y: MAP_SIZE - 600 }));
  g.lineTo(...px({ x: MAP_SIZE - 600, y: 600 }));
  g.stroke();
  g.strokeStyle = 'rgba(140,210,255,0.18)';
  g.lineWidth = 20;
  g.stroke();
  g.restore();
  // fosses
  for (const pit of [PITS.dragon, PITS.leviathan]) {
    const [x, y] = px(pit.pos);
    g.fillStyle = 'rgba(40,90,120,0.6)';
    g.beginPath();
    g.arc(x, y, pit.radius * S, 0, Math.PI * 2);
    g.fill();
  }
  // voies
  g.lineCap = 'round';
  g.lineJoin = 'round';
  for (const path of Object.values(LANE_PATHS)) {
    g.strokeStyle = 'rgba(214,190,140,0.32)';
    g.lineWidth = 22;
    g.beginPath();
    path.forEach((p, i) => (i ? g.lineTo(...px(p)) : g.moveTo(...px(p))));
    g.stroke();
    g.strokeStyle = 'rgba(240,220,170,0.12)';
    g.lineWidth = 8;
    g.stroke();
  }
  // bases
  for (const t of ['blue', 'red'] as Team[]) {
    const [x, y] = px(NEXUS[t]);
    const grad = g.createRadialGradient(x, y, 4, x, y, 2600 * S);
    grad.addColorStop(0, t === 'blue' ? 'rgba(78,162,255,0.35)' : 'rgba(255,90,90,0.35)');
    grad.addColorStop(1, 'rgba(0,0,0,0)');
    g.fillStyle = grad;
    g.beginPath();
    g.arc(x, y, 2600 * S, 0, Math.PI * 2);
    g.fill();
    const [fx, fy] = px(FOUNTAINS[t]);
    g.fillStyle = t === 'blue' ? 'rgba(78,162,255,0.5)' : 'rgba(255,90,90,0.5)';
    g.beginPath();
    g.arc(fx, fy, 900 * S * 0.6, 0, Math.PI * 2);
    g.fill();
  }
  // brousses
  g.fillStyle = 'rgba(70,140,60,0.55)';
  for (const b of BRUSHES) poly(g, b);
  // murs
  g.fillStyle = '#0c120e';
  g.strokeStyle = 'rgba(160,150,120,0.35)';
  g.lineWidth = 1.5;
  for (const w of WALLS) {
    poly(g, w);
    g.stroke();
  }
  // bord
  g.strokeStyle = 'rgba(0,0,0,0.6)';
  g.lineWidth = 6;
  g.strokeRect(0, 0, RES, RES);
  return c;
}

function poly(g: CanvasRenderingContext2D, pts: Vec2[]): void {
  g.beginPath();
  pts.forEach((p, i) => (i ? g.lineTo(...px(p)) : g.moveTo(...px(p))));
  g.closePath();
  g.fill();
}

interface Ping {
  pos: Vec2;
  t: number;
  kind: string;
}

export function Minimap() {
  const ref = useRef<HTMLCanvasElement>(null);
  const timers = useSelect((s) => s.snapshot?.objectiveTimers, (a, b) => a === b);
  const time = useSelect((s) => Math.floor(s.snapshot?.time ?? 0));
  const pings = useRef<Ping[]>([]);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const g = canvas.getContext('2d')!;
    const staticLayer = drawStatic();
    let raf = 0;
    let lastVersion = -1;
    let lastDraw = 0;
    const draw = () => {
      raf = requestAnimationFrame(draw);
      const now = performance.now();
      // la caméra bouge à chaque frame : on redessine au plus ~30 fois/s
      if (now - lastDraw < 33) return;
      lastDraw = now;
      lastVersion = worldTick.state.version;
      g.drawImage(staticLayer, 0, 0);
      drawFog(g);
      drawDynamic(g, now, pings.current);
    };
    raf = requestAnimationFrame(draw);
    let lastSnap = store.state.snapshot;
    const offPing = store.subscribe((s) => {
      if (s.snapshot === lastSnap) return;
      lastSnap = s.snapshot;
      for (const e of s.snapshot?.events ?? []) if (e.type === 'ping') pings.current.push({ pos: e.pos, t: performance.now(), kind: e.kind });
    });
    return () => {
      cancelAnimationFrame(raf);
      offPing();
    };
  }, []);

  const toGame = (e: MouseEvent): Vec2 => {
    const r = (e.currentTarget as HTMLCanvasElement).getBoundingClientRect();
    return { x: ((e.clientX - r.left) / r.width) * MAP_SIZE, y: (1 - (e.clientY - r.top) / r.height) * MAP_SIZE };
  };
  const onDown = (e: PointerEvent) => {
    e.preventDefault();
    const pos = toGame(e);
    if (e.button === 2) {
      net.send({ t: 'move', pos });
      pings.current.push({ pos, t: performance.now(), kind: 'move' });
      return;
    }
    if (e.altKey) {
      net.send({ t: 'ping', pos, kind: 'alert' });
      pings.current.push({ pos, t: performance.now(), kind: 'alert' });
      return;
    }
    focusCamera(pos);
    (e.currentTarget as HTMLCanvasElement).setPointerCapture(e.pointerId);
  };
  const onMove = (e: PointerEvent) => {
    if (e.buttons & 1 && !e.altKey) focusCamera(toGame(e));
  };

  const timerList = settings.state.showTimers && timers ? Object.entries(timers).filter(([k, at]) => k in EPIC && at > time).sort((a, b) => a[1] - b[1]) : [];

  return (
    <div class="lov-minimap">
      {timerList.length > 0 && (
        <div class="lov-obj-timers">
          {timerList.map(([k, at]) => (
            <span key={k} class={`t-${k} ${at - time <= 30 ? 'soon' : ''}`}>
              <Glyph name={EPIC[k]!} size={13} />
              {MONSTER_LABEL[k as keyof typeof MONSTER_LABEL] ?? k}
              <b>{formatClock(at - time)}</b>
            </span>
          ))}
        </div>
      )}
      <canvas ref={ref} width={RES} height={RES} onPointerDown={onDown} onPointerMove={onMove} onContextMenu={(e) => e.preventDefault()} />
    </div>
  );
}

/** Brouillard de guerre : canvas de visibilité de claude-client (blanc = visible), appliqué en « multiply ». */
function drawFog(g: CanvasRenderingContext2D): void {
  const fog = getRenderer()?.fog.canvas;
  if (!fog) return;
  g.save();
  g.globalCompositeOperation = 'multiply';
  g.globalAlpha = 0.62;
  g.drawImage(fog, 0, 0, RES, RES);
  g.restore();
}

function drawDynamic(g: CanvasRenderingContext2D, now: number, pings: Ping[]): void {
  const team = myTeam();
  const me = selfEntity();
  let hasStructures = false;
  const minions: [number, number, string][] = [];
  const champs: { x: number; y: number; id: string; team: string; self: boolean; dead: boolean }[] = [];

  entities.forEach((e) => {
    const [x, y] = px(e);
    switch (e.kind) {
      case 'tower':
      case 'inhibitor':
      case 'nexus':
        hasStructures = true;
        drawStructure(g, e.kind, x, y, e.team === 'blue' ? COL.blue : COL.red, !!e.dead || e.hp <= 0);
        break;
      case 'minion':
        if (!e.dead) minions.push([x, y, e.team === 'blue' ? COL.blue : COL.red]);
        break;
      case 'monster':
        if (!e.dead) drawMonster(g, x, y, e.subtype ?? '');
        break;
      case 'ward':
        g.fillStyle = e.team === team ? '#ffe680' : '#ff7070';
        g.beginPath();
        g.arc(x, y, 4, 0, Math.PI * 2);
        g.fill();
        break;
      case 'champion':
        champs.push({ x, y, id: e.subtype ?? '', team: e.team, self: e.id === me?.id, dead: !!e.dead });
        break;
      default:
        break;
    }
  });

  // repli : structures statiques si le serveur ne les envoie pas encore
  if (!hasStructures) {
    for (const t of TOWERS) drawStructure(g, 'tower', ...px(t.pos), t.team === 'blue' ? COL.blue : COL.red, false);
    for (const i of INHIBITORS) drawStructure(g, 'inhibitor', ...px(i.pos), i.team === 'blue' ? COL.blue : COL.red, false);
    for (const t of ['blue', 'red'] as Team[]) drawStructure(g, 'nexus', ...px(NEXUS[t]), t === 'blue' ? COL.blue : COL.red, false);
    for (const c of CAMPS) {
      const [x, y] = px(c.pos);
      g.fillStyle = 'rgba(232,196,106,0.35)';
      g.beginPath();
      g.arc(x, y, 4, 0, Math.PI * 2);
      g.fill();
    }
  }

  for (const [x, y, c] of minions) {
    g.fillStyle = c;
    g.fillRect(x - 2.5, y - 2.5, 5, 5);
  }

  // cadre caméra : emprise réelle de la vue (trapèze, mise à jour en silence par claude-client)
  const quad = store.state.cameraView?.quad;
  if (quad) {
    g.strokeStyle = 'rgba(255,255,255,0.9)';
    g.lineWidth = 2;
    g.beginPath();
    quad.forEach((p, i) => (i ? g.lineTo(...px(p)) : g.moveTo(...px(p))));
    g.closePath();
    g.stroke();
  }

  // camps de jungle en réapparition : petit compte à rebours sur la carte
  const timers = store.state.snapshot?.objectiveTimers;
  const now0 = store.state.snapshot?.time ?? 0;
  if (timers && settings.state.showTimers) {
    g.font = '700 15px Inter, sans-serif';
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    for (const [id, at] of Object.entries(timers)) {
      const pos = CAMP_POS.get(id);
      if (!pos || at <= now0) continue;
      const [x, y] = px(pos);
      const left = at - now0;
      g.fillStyle = 'rgba(5,7,15,0.82)';
      g.beginPath();
      g.arc(x, y, 13, 0, Math.PI * 2);
      g.fill();
      g.strokeStyle = left <= 15 ? '#f6dc94' : 'rgba(232,196,106,0.5)';
      g.lineWidth = 2;
      g.stroke();
      g.fillStyle = left <= 15 ? '#f6dc94' : '#cfc9b8';
      g.fillText(left >= 60 ? `${Math.ceil(left / 60)}m` : String(Math.ceil(left)), x, y + 1);
    }
  }

  // champions (le joueur par-dessus)
  champs.sort((a, b) => Number(a.self) - Number(b.self));
  for (const c of champs) drawChamp(g, c);

  // pings
  for (let i = pings.length - 1; i >= 0; i--) {
    const p = pings[i]!;
    const age = (now - p.t) / 1000;
    if (age > 1.6) {
      pings.splice(i, 1);
      continue;
    }
    const [x, y] = px(p.pos);
    g.strokeStyle = p.kind === 'move' ? `rgba(120,255,160,${1 - age / 1.6})` : `rgba(255,220,90,${1 - age / 1.6})`;
    g.lineWidth = 3;
    for (let k = 0; k < 2; k++) {
      g.beginPath();
      g.arc(x, y, 6 + ((age * 40 + k * 14) % 30), 0, Math.PI * 2);
      g.stroke();
    }
  }
}

function drawStructure(g: CanvasRenderingContext2D, kind: string, x: number, y: number, col: string, dead: boolean): void {
  g.save();
  g.globalAlpha = dead ? 0.3 : 1;
  g.fillStyle = col;
  g.strokeStyle = '#0a0a0a';
  g.lineWidth = 2;
  g.beginPath();
  if (kind === 'nexus') {
    g.moveTo(x, y - 13);
    g.lineTo(x + 13, y);
    g.lineTo(x, y + 13);
    g.lineTo(x - 13, y);
  } else if (kind === 'inhibitor') {
    g.arc(x, y, 7, 0, Math.PI * 2);
  } else {
    g.moveTo(x, y - 9);
    g.lineTo(x + 7, y + 6);
    g.lineTo(x - 7, y + 6);
  }
  g.closePath();
  g.fill();
  g.stroke();
  g.restore();
}

function drawMonster(g: CanvasRenderingContext2D, x: number, y: number, sub: string): void {
  const epic = sub === 'dragon' || sub === 'elder_dragon' || sub === 'leviathan' || sub === 'herald';
  g.fillStyle = epic ? '#c48bff' : COL.neutral;
  g.strokeStyle = '#0a0a0a';
  g.lineWidth = 2;
  g.beginPath();
  g.arc(x, y, epic ? 9 : 5, 0, Math.PI * 2);
  g.fill();
  g.stroke();
  if (epic) {
    g.fillStyle = '#0a0a0a';
    g.font = 'bold 10px Inter, sans-serif';
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    g.fillText(sub === 'leviathan' ? 'L' : sub === 'herald' ? 'C' : 'D', x, y + 0.5);
  }
}

function drawChamp(g: CanvasRenderingContext2D, c: { x: number; y: number; id: string; team: string; self: boolean; dead: boolean }): void {
  const pal = CHAMPIONS[c.id]?.palette;
  const r = c.self ? 15 : 13;
  g.save();
  g.globalAlpha = c.dead ? 0.35 : 1;
  g.fillStyle = pal?.secondary ?? '#333';
  g.beginPath();
  g.arc(c.x, c.y, r, 0, Math.PI * 2);
  g.fill();
  const grad = g.createRadialGradient(c.x, c.y - 3, 1, c.x, c.y, r);
  grad.addColorStop(0, pal?.primary ?? '#999');
  grad.addColorStop(1, pal?.secondary ?? '#333');
  g.fillStyle = grad;
  g.fill();
  g.lineWidth = c.self ? 3.5 : 3;
  g.strokeStyle = c.self ? COL.self : c.team === 'blue' ? COL.blue : COL.red;
  g.stroke();
  g.fillStyle = pal?.accent ?? '#fff';
  g.font = `800 ${c.self ? 15 : 13}px Cinzel, serif`;
  g.textAlign = 'center';
  g.textBaseline = 'middle';
  g.fillText((CHAMPIONS[c.id]?.name ?? '?').charAt(0), c.x, c.y + 1);
  g.restore();
}
