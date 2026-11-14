/**
 * Contrôles clavier/souris (PLAN §10). Propriétaire : claude-client.
 *
 * - Clic droit : sur un ennemi = attaque, sur le sol = déplacement (+ marqueur). Maintenu = suit le curseur.
 * - A puis clic gauche : attaque-déplacement (quick-cast : immédiat).
 * - Q W E R : cast normal (touche maintenue = indicateur, relâcher la touche ou clic gauche pour lancer,
 *   clic droit/Échap pour annuler) ou quick-cast selon les paramètres ; Maj+touche inverse le mode.
 *   Ctrl+QWER : monter le sort.
 * - D / F : sorts d'invocateur. 1 2 3 5 6 7 : objets actifs. 4 : balise. B : rappel. S : stop.
 * - Espace (maintenu) : centrer la caméra. Y : verrouiller/déverrouiller. Flèches : déplacer la caméra.
 * - P : boutique. Tab (maintenu) : scores. C : stats. Échap : annuler / fermer / paramètres.
 * - Alt+clic gauche : ping.
 */
import { CHAMPIONS, SUMMONERS, type AbilitySlot, type EntityId, type EntitySnapshot, type SummonerSlot, type Targeting as TargetingType, type Vec2 } from '@lov/shared';
import { net, snapshots } from '../net/client.ts';
import type { GameRenderer } from '../render/GameRenderer.ts';
import { store, type Targeting } from '../state.ts';
import { digitAction, isAttackable, needsTarget } from './rules.ts';

const ATTACK_CURSOR = cursorSvg('#ff5a4a');
const TARGET_CURSOR = cursorSvg('#7fd8ff');

function cursorSvg(color: string): string {
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'><g fill='none' stroke='${color}' stroke-width='2.5'><circle cx='16' cy='16' r='9'/><path d='M16 2v8M16 22v8M2 16h8M22 16h8'/></g><circle cx='16' cy='16' r='2' fill='${color}'/></svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}") 16 16, crosshair`;
}

export function installControls(renderer: GameRenderer): void {
  const canvas = renderer.renderer.domElement;
  const cursor: Vec2 = { x: 0, y: 0 };
  let hasCursor = false;
  let mouseX = 0;
  let mouseY = 0;
  let rightHeld = false;
  /** Touche physique (e.code) qui a ouvert l'indicateur de sort en cours : la relâcher lance le sort. */
  let castHeldCode: string | null = null;
  let lastFollowSend = 0;
  const lastFollowPos: Vec2 = { x: 0, y: 0 };

  const inGame = () => !!store.state.game && !store.state.result;
  const myTeam = () => store.state.game?.team;
  const self = () => store.state.snapshot?.self;
  const hoverEntity = (): EntitySnapshot | null => {
    const id = store.state.hover;
    return id === null ? null : (snapshots.entities.get(id) ?? null);
  };
  const enemyHover = (): EntityId | null => {
    const e = hoverEntity();
    const t = myTeam();
    return e && t && isAttackable(e, t) ? e.id : null;
  };

  const setTargeting = (t: Targeting) => {
    if (!t) castHeldCode = null;
    if (store.state.targeting !== t) store.set({ targeting: t });
    updateCursor();
  };

  const updateCursor = () => {
    canvas.style.cursor = store.state.targeting ? TARGET_CURSOR : enemyHover() !== null ? ATTACK_CURSOR : 'default';
  };

  const refreshPointer = () => {
    if (!inGame()) return;
    const g = renderer.pickGround(mouseX, mouseY, cursor);
    hasCursor = !!g;
    store.setQuiet('cursorGame', hasCursor ? cursor : null);
    const hover = renderer.pickEntity(mouseX, mouseY);
    if (hover !== store.state.hover) {
      store.set({ hover });
      updateCursor();
    }
  };

  // Le picking dépend de la caméra qui bouge : on le rafraîchit à chaque frame.
  renderer.onFrame(() => {
    refreshPointer();
    if (rightHeld && hasCursor && inGame()) {
      const now = performance.now();
      if (now - lastFollowSend > 120 && Math.hypot(cursor.x - lastFollowPos.x, cursor.y - lastFollowPos.y) > 40) {
        lastFollowSend = now;
        lastFollowPos.x = cursor.x;
        lastFollowPos.y = cursor.y;
        net.send({ t: 'move', pos: { x: cursor.x, y: cursor.y } });
        renderer.predictor.command(cursor, (store.state.rtt ?? 80) / 1000);
      }
    }
  });

  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
  canvas.addEventListener('pointermove', (e) => {
    mouseX = e.clientX;
    mouseY = e.clientY;
    renderer.cam.setMouse(e.clientX, e.clientY);
  });
  document.addEventListener('pointerleave', () => renderer.cam.clearMouse());
  addEventListener('blur', () => {
    renderer.cam.clearMouse();
    rightHeld = false;
    castHeldCode = null;
    renderer.cam.centerHeld = false;
    if (store.state.scoreboardOpen) store.set({ scoreboardOpen: false });
  });
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    renderer.cam.zoomBy(e.deltaY);
  }, { passive: false });

  addEventListener('pointerup', (e) => {
    if (e.button === 2) rightHeld = false;
  });

  canvas.addEventListener('pointerdown', (e) => {
    if (!inGame()) return;
    mouseX = e.clientX;
    mouseY = e.clientY;
    refreshPointer();
    const pos = hasCursor ? { x: cursor.x, y: cursor.y } : null;

    if (e.button === 0) {
      if (e.altKey && pos) {
        net.send({ t: 'ping', pos, kind: enemyHover() !== null ? 'danger' : 'generic' });
        return;
      }
      const t = store.state.targeting;
      if (t) confirmTargeting(t, pos);
      return;
    }

    if (e.button === 2) {
      if (store.state.targeting) setTargeting(null);
      const target = enemyHover();
      if (target !== null) {
        net.send({ t: 'attack', targetId: target });
        renderer.predictor.cancel();
        const te = hoverEntity();
        if (te) renderer.markers.spawn(te, 0xff4040);
        return;
      }
      if (pos) {
        net.send({ t: 'move', pos });
        renderer.localMoveCommand(pos);
        rightHeld = true;
        lastFollowSend = performance.now();
        lastFollowPos.x = pos.x;
        lastFollowPos.y = pos.y;
      }
    }
  });

  function confirmTargeting(t: NonNullable<Targeting>, pos: Vec2 | null): void {
    const hover = store.state.hover ?? undefined;
    switch (t.kind) {
      case 'attackMove': {
        const target = enemyHover();
        if (target !== null) net.send({ t: 'attack', targetId: target });
        else if (pos) {
          net.send({ t: 'attackMove', pos });
          renderer.localMoveCommand(pos, true);
        }
        break;
      }
      case 'ward':
        if (pos) net.send({ t: 'placeWard', pos });
        break;
      case 'ability': {
        const targeting = abilityTargeting(t.slot);
        if (targeting && needsTarget(targeting) && hover === undefined) return; // attendre un clic sur une unité
        net.send({ t: 'cast', slot: t.slot, targetPos: pos ?? undefined, targetId: hover });
        renderer.predictor.cancel();
        break;
      }
      case 'summoner': {
        const targeting = summonerTargeting(t.slot);
        if (targeting && needsTarget(targeting) && hover === undefined) return;
        net.send({ t: 'summoner', slot: t.slot, targetPos: pos ?? undefined, targetId: hover });
        break;
      }
    }
    setTargeting(null);
  }

  function abilityTargeting(slot: AbilitySlot): TargetingType | null {
    const champ = store.state.selfEntity?.subtype;
    return champ ? (CHAMPIONS[champ]?.abilities[slot].targeting ?? null) : null;
  }
  function summonerTargeting(slot: SummonerSlot): TargetingType | null {
    const id = self()?.summoners[slot];
    return id ? SUMMONERS[id].targeting : null;
  }

  /** QWER / DF : cast normal ou rapide. */
  function castKey(t: NonNullable<Targeting>, invertQuick: boolean, code: string): void {
    const st = self();
    const targeting = t.kind === 'ability' ? abilityTargeting(t.slot) : t.kind === 'summoner' ? summonerTargeting(t.slot) : null;
    if (st && t.kind === 'ability' && st.abilityLevels[t.slot] === 0) return;
    if (st && (t.kind === 'ability' || t.kind === 'summoner') && st.cooldowns[t.slot] > 0) return;
    const quick = store.state.settings.quickCast !== invertQuick;
    if (targeting === 'self' || quick) {
      confirmTargeting(t, hasCursor ? { x: cursor.x, y: cursor.y } : null);
      return;
    }
    setTargeting(t);
    castHeldCode = code;
  }

  addEventListener('keydown', (e) => {
    const el = e.target as HTMLElement | null;
    if (el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable)) return;
    if (!inGame()) return;
    const code = e.code;

    if (e.key === 'Tab') {
      e.preventDefault();
      if (!store.state.scoreboardOpen) store.set({ scoreboardOpen: true });
      return;
    }
    if (code.startsWith('Arrow')) {
      renderer.cam.setKey(code === 'ArrowLeft' ? 'left' : code === 'ArrowRight' ? 'right' : code === 'ArrowUp' ? 'up' : 'down', true);
      e.preventDefault();
      return;
    }
    if (code === 'Space') {
      renderer.cam.centerHeld = true;
      e.preventDefault();
      return;
    }
    if (e.repeat) return;

    // rangée du haut : objets (1 2 3 5 6 7) et balise (4), par position physique
    const digit = digitAction(code);
    if (digit) {
      e.preventDefault();
      if (digit.kind === 'ward') setTargeting({ kind: 'ward' });
      else net.send({ t: 'useItem', slot: digit.slot, targetPos: hasCursor ? { x: cursor.x, y: cursor.y } : undefined });
      return;
    }
    // on se fie à e.code (position physique) pour QWER/DF... sauf en AZERTY où l'utilisateur attend les lettres :
    // e.key est donc prioritaire pour les lettres.
    const key = e.key.length === 1 ? e.key.toUpperCase() : '';
    if (key === 'Q' || key === 'W' || key === 'E' || key === 'R') {
      e.preventDefault();
      if (e.ctrlKey) net.send({ t: 'levelAbility', slot: key });
      else castKey({ kind: 'ability', slot: key }, e.shiftKey, code);
      return;
    }
    if (key === 'D' || key === 'F') {
      e.preventDefault();
      castKey({ kind: 'summoner', slot: key }, e.shiftKey, code);
      return;
    }
    switch (key) {
      case 'A':
        if (store.state.settings.quickCast) confirmTargeting({ kind: 'attackMove' }, hasCursor ? { x: cursor.x, y: cursor.y } : null);
        else setTargeting({ kind: 'attackMove' });
        return;
      case 'S':
        net.send({ t: 'stop' });
        renderer.predictor.cancel();
        setTargeting(null);
        return;
      case 'B':
        net.send({ t: 'recall' });
        renderer.predictor.cancel();
        return;
      case 'Y':
        store.set({ cameraLocked: !store.state.cameraLocked });
        return;
      case 'P':
        store.set({ shopOpen: !store.state.shopOpen });
        return;
      case 'C':
        store.set({ statsOpen: !store.state.statsOpen });
        return;
    }
    if (e.key === 'Escape') {
      if (store.state.targeting) setTargeting(null);
      else if (store.state.shopOpen) store.set({ shopOpen: false });
      else store.set({ settingsOpen: !store.state.settingsOpen });
    }
  });

  addEventListener('keyup', (e) => {
    // cast au relâchement : la touche qui a ouvert l'indicateur lance le sort (si toujours actif)
    if (castHeldCode !== null && e.code === castHeldCode) {
      castHeldCode = null;
      const t = store.state.targeting;
      if (t && inGame() && (t.kind === 'ability' || t.kind === 'summoner')) {
        refreshPointer();
        confirmTargeting(t, hasCursor ? { x: cursor.x, y: cursor.y } : null);
      }
      return;
    }
    if (e.key === 'Tab') {
      if (store.state.scoreboardOpen) store.set({ scoreboardOpen: false });
      return;
    }
    if (e.code.startsWith('Arrow')) {
      renderer.cam.setKey(e.code === 'ArrowLeft' ? 'left' : e.code === 'ArrowRight' ? 'right' : e.code === 'ArrowUp' ? 'up' : 'down', false);
      return;
    }
    if (e.code === 'Space') renderer.cam.centerHeld = false;
  });
}
