/**
 * Indicateurs de visée et de portée — API pour claude-client (input) :
 *
 *   import { fxIndicators } from '../fx/index.ts';
 *   fxIndicators.showAbility(def, casterPos, cursorPos); // à chaque mouvement de souris en mode visée
 *   fxIndicators.showRange(casterPos, attackRange);      // cercle de portée d'AA au survol
 *   fxIndicators.hide();
 *
 * Coordonnées de jeu. Les décals sont créés une fois et réutilisés ; seuls de petits objets de forme sont alloués par appel.
 */
import * as THREE from 'three';
import { RENDER_SCALE, type AbilityDef, type SummonerDef, type Vec2 } from '@lov/shared';
import { GroundDecal } from '../shaders/decal.ts';
import { aimFor, aimForSummoner, type AimIndicator, type AimShape } from './aim.ts';

const RANGE_COLOR = new THREE.Color(0.55, 0.8, 1.2);
const AIM_COLOR = new THREE.Color(0.7, 0.9, 1.4);
const _a = new THREE.Vector3();
const _b = new THREE.Vector3();

const toW = (p: Vec2, out: THREE.Vector3): THREE.Vector3 => out.set(p.x * RENDER_SCALE, 0, -p.y * RENDER_SCALE);

export class FxIndicators {
  private root: THREE.Object3D | null = null;
  private rangeDecal: GroundDecal | null = null;
  private aimDecal: GroundDecal | null = null;

  /** Appelé par installFx. Avant cela, toutes les méthodes sont sans effet. */
  attach(root: THREE.Object3D): void {
    this.root = root;
    this.rangeDecal = new GroundDecal().setColor(RANGE_COLOR);
    this.rangeDecal.u.uFill.value = 0;
    this.rangeDecal.u.uEdge.value = 0.05;
    this.rangeDecal.u.uOpacity.value = 0.55;
    this.aimDecal = new GroundDecal().setColor(AIM_COLOR);
    this.aimDecal.u.uFill.value = 0.22;
    this.aimDecal.u.uEdge.value = 0.06;
    this.aimDecal.u.uOpacity.value = 0.8;
    for (const d of [this.rangeDecal, this.aimDecal]) {
      d.mesh.visible = false;
      d.mesh.renderOrder = 6;
      root.add(d.mesh);
    }
  }

  showRange(center: Vec2, radius: number): void {
    const d = this.rangeDecal;
    if (!d) return;
    d.circle(toW(center, _a), radius * RENDER_SCALE);
    d.mesh.visible = true;
  }

  hideRange(): void {
    if (this.rangeDecal) this.rangeDecal.mesh.visible = false;
  }

  showAim(shape: AimShape | null): void {
    const d = this.aimDecal;
    if (!d) return;
    if (!shape) {
      d.mesh.visible = false;
      return;
    }
    if (shape.kind === 'circle') d.circle(toW(shape.center, _a), shape.radius * RENDER_SCALE);
    else if (shape.kind === 'line') d.rect(toW(shape.from, _a), toW(shape.to, _b), shape.width * RENDER_SCALE);
    else {
      toW(shape.from, _a);
      toW(shape.to, _b);
      d.cone(_a, _b, _a.distanceTo(_b), shape.halfAngle);
    }
    d.mesh.visible = true;
  }

  show(ind: AimIndicator, caster: Vec2): void {
    if (ind.range !== null) this.showRange(caster, ind.range);
    else this.hideRange();
    this.showAim(ind.aim);
  }

  /** Indicateur complet d'un sort de champion (portée + forme selon `targeting`). */
  showAbility(def: AbilityDef, caster: Vec2, cursor: Vec2): void {
    this.show(aimFor(def, caster, cursor), caster);
  }

  showSummoner(def: SummonerDef, caster: Vec2, cursor: Vec2): void {
    this.show(aimForSummoner(def, caster, cursor), caster);
  }

  hide(): void {
    this.hideRange();
    this.showAim(null);
  }

  get attached(): boolean {
    return this.root !== null;
  }
}

export const fxIndicators = new FxIndicators();
