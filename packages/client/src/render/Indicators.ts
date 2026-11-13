/**
 * Indicateurs au sol : portée d'auto-attaque et visée des sorts. Propriétaire : claude-client.
 *
 * Contrat avec claude-fx : `registerIndicatorRenderer(r)` remplace le visuel par défaut. claude-client calcule
 * l'état de visée (`AimState`, objet réutilisé) à chaque frame ; le renderer enregistré l'affiche.
 */
import * as THREE from 'three';
import { CHAMPIONS, SUMMONERS, type AbilityDef, type AbilitySlot, type EntityId, type SummonerDef, type SummonerSlot, type Targeting, type Vec2 } from '@lov/shared';
import type { Targeting as TargetingState } from '../state.ts';
import { S } from './coords.ts';
import { terrainHeight } from './map/geometry.ts';

export interface AimState {
  /** Un ciblage est en cours (sort, invocateur, attaque-déplacement). */
  active: boolean;
  kind: 'ability' | 'summoner' | 'attackMove' | 'ward' | null;
  slot: AbilitySlot | SummonerSlot | null;
  /** Type de ciblage du sort (schema.ts). attackMove/ward : 'ground'. */
  targeting: Targeting | null;
  /** Identifiant du sort (abilityId / summonerId) et clé FX, pour un visuel spécifique. */
  abilityId: string | null;
  fxKey: string | null;
  /** Définition complète du sort (champion ou invocateur), null sinon. */
  def: AbilityDef | SummonerDef | null;
  /** Position du lanceur (affichée/prédite) et du curseur, coordonnées jeu. */
  origin: Vec2;
  cursor: Vec2;
  /** Point visé borné à la portée. */
  clamped: Vec2;
  range: number;
  radius: number;
  width: number;
  hover: EntityId | null;
  /** Portée d'auto-attaque à afficher (0 = masquée). */
  attackRange: number;
}

export interface IndicatorRenderer {
  update(aim: Readonly<AimState>, dt: number): void;
  dispose?(): void;
}

let custom: IndicatorRenderer | null = null;
/** claude-fx : remplace les indicateurs par défaut. */
export function registerIndicatorRenderer(r: IndicatorRenderer): void {
  custom = r;
}

export function newAimState(): AimState {
  return { active: false, kind: null, slot: null, targeting: null, abilityId: null, fxKey: null, def: null, origin: { x: 0, y: 0 }, cursor: { x: 0, y: 0 }, clamped: { x: 0, y: 0 }, range: 0, radius: 0, width: 0, hover: null, attackRange: 0 };
}

/** Remplit `aim` à partir de l'état de ciblage (fonction pure, testable). */
export function computeAim(aim: AimState, t: TargetingState, championId: string | undefined, summoners: Record<SummonerSlot, string> | undefined, origin: Vec2, cursor: Vec2 | null, hover: EntityId | null, attackRange: number): AimState {
  aim.active = !!t;
  aim.kind = t?.kind ?? null;
  aim.slot = t && 'slot' in t ? t.slot : null;
  aim.targeting = null;
  aim.abilityId = null;
  aim.fxKey = null;
  aim.def = null;
  aim.range = 0;
  aim.radius = 0;
  aim.width = 0;
  aim.hover = hover;
  aim.origin.x = origin.x;
  aim.origin.y = origin.y;
  if (cursor) {
    aim.cursor.x = cursor.x;
    aim.cursor.y = cursor.y;
  }
  aim.attackRange = 0;
  if (t?.kind === 'ability' && championId) {
    const a = CHAMPIONS[championId]?.abilities[t.slot];
    if (a) {
      aim.def = a;
      aim.targeting = a.targeting;
      aim.abilityId = a.id;
      aim.fxKey = a.fxKey;
      aim.range = a.range;
      aim.radius = a.radius ?? 0;
      aim.width = a.width ?? 0;
    }
  } else if (t?.kind === 'summoner' && summoners) {
    const def = SUMMONERS[summoners[t.slot] as keyof typeof SUMMONERS];
    if (def) {
      aim.def = def;
      aim.targeting = def.targeting;
      aim.abilityId = def.id;
      aim.fxKey = def.fxKey;
      aim.range = def.range;
    }
  } else if (t?.kind === 'attackMove') {
    aim.targeting = 'ground';
    aim.attackRange = attackRange;
  } else if (t?.kind === 'ward') {
    aim.targeting = 'ground';
    aim.range = 600;
  }
  // point visé borné à la portée
  const dx = aim.cursor.x - origin.x;
  const dy = aim.cursor.y - origin.y;
  const d = Math.hypot(dx, dy);
  const lim = aim.range > 0 && aim.range < 20000 ? aim.range : Infinity;
  const k = d > lim ? lim / d : 1;
  aim.clamped.x = origin.x + dx * k;
  aim.clamped.y = origin.y + dy * k;
  return aim;
}

// ---------------------------------------------------------------- visuel par défaut

const COLOR = 0x7fd8ff;

export class DefaultIndicators implements IndicatorRenderer {
  private group = new THREE.Group();
  private rangeRing: THREE.Mesh;
  private aaRing: THREE.Mesh;
  private area: THREE.Mesh;
  private line: THREE.Mesh;
  private cone: THREE.Mesh;
  private t = 0;
  private lineMat: THREE.MeshBasicMaterial;

  constructor(scene: THREE.Scene) {
    const ringMat = (opacity: number) => new THREE.MeshBasicMaterial({ color: COLOR, transparent: true, opacity, depthWrite: false, side: THREE.DoubleSide });
    this.rangeRing = new THREE.Mesh(new THREE.RingGeometry(0.985, 1, 96).rotateX(-Math.PI / 2), ringMat(0.55));
    this.aaRing = new THREE.Mesh(new THREE.RingGeometry(0.98, 1, 96).rotateX(-Math.PI / 2), new THREE.MeshBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.45, depthWrite: false }));
    this.area = new THREE.Mesh(new THREE.CircleGeometry(1, 48).rotateX(-Math.PI / 2), ringMat(0.22));
    this.lineMat = ringMat(0.28);
    // rectangle unitaire de 0 à 1 le long de +x
    this.line = new THREE.Mesh(new THREE.PlaneGeometry(1, 1).translate(0.5, 0, 0).rotateX(-Math.PI / 2), this.lineMat);
    this.cone = new THREE.Mesh(new THREE.CircleGeometry(1, 24, -Math.PI / 6, Math.PI / 3).rotateX(-Math.PI / 2), ringMat(0.25));
    for (const m of [this.rangeRing, this.aaRing, this.area, this.line, this.cone]) {
      m.renderOrder = 4;
      m.visible = false;
      this.group.add(m);
    }
    scene.add(this.group);
  }

  private place(m: THREE.Object3D, p: Vec2, lift = 0.12): void {
    m.position.set(p.x * S, terrainHeight(p.x, p.y) * S + lift, -p.y * S);
  }

  update(aim: Readonly<AimState>, dt: number): void {
    this.t += dt;
    for (const m of this.group.children) m.visible = false;
    if (aim.attackRange > 0) {
      this.aaRing.visible = true;
      this.place(this.aaRing, aim.origin);
      this.aaRing.scale.setScalar(aim.attackRange * S);
    }
    if (!aim.active || !aim.targeting || aim.kind === 'attackMove') return;
    if (aim.range > 0 && aim.range < 20000) {
      this.rangeRing.visible = true;
      this.place(this.rangeRing, aim.origin);
      this.rangeRing.scale.setScalar(aim.range * S);
    }
    const ang = Math.atan2(aim.cursor.y - aim.origin.y, aim.cursor.x - aim.origin.x);
    switch (aim.targeting) {
      case 'skillshot':
      case 'dash': {
        const len = aim.targeting === 'dash' ? Math.hypot(aim.clamped.x - aim.origin.x, aim.clamped.y - aim.origin.y) : aim.range;
        this.line.visible = true;
        this.place(this.line, aim.origin, 0.13);
        this.line.rotation.y = ang;
        this.line.scale.set(len * S, 1, Math.max(aim.width, 60) * S);
        this.lineMat.opacity = 0.24 + Math.sin(this.t * 6) * 0.04;
        if (aim.radius > 0) {
          this.area.visible = true;
          this.place(this.area, aim.clamped, 0.14);
          this.area.scale.setScalar(aim.radius * S);
        }
        break;
      }
      case 'ground':
        this.area.visible = true;
        this.place(this.area, aim.clamped, 0.14);
        this.area.scale.setScalar(Math.max(aim.radius, 80) * S);
        break;
      case 'cone':
        this.cone.visible = true;
        this.place(this.cone, aim.origin, 0.13);
        this.cone.rotation.y = ang;
        this.cone.scale.setScalar((aim.range || aim.radius) * S);
        break;
      default:
        break;
    }
  }
}

/** Orchestration : visuel personnalisé (claude-fx) s'il existe, sinon visuel par défaut. */
export class Indicators {
  private fallback: DefaultIndicators;
  constructor(scene: THREE.Scene) {
    this.fallback = new DefaultIndicators(scene);
  }
  update(aim: Readonly<AimState>, dt: number): void {
    if (custom) {
      this.fallback.update(newAimStateHidden, dt);
      custom.update(aim, dt);
    } else this.fallback.update(aim, dt);
  }
}
const newAimStateHidden = newAimState();
