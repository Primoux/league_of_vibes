/**
 * Squelette procédural : hiérarchie d'Object3D (segments rigides) pilotée par une Pose.
 * Le modèle regarde vers +x local, la droite est +z (voir anim/pose.ts).
 */
import * as THREE from 'three';
import { BODY_OFFSET, J, JOINT_COUNT, type Pose } from './anim/pose.ts';

export interface RigSpec {
  /** Hauteur de la hanche. */
  hipY: number;
  /** Demi-écart des hanches (jambes). */
  hipW: number;
  thigh: number;
  shin: number;
  /** Hanche → épaules. */
  spine: number;
  /** Demi-largeur d'épaules. */
  shoulderW: number;
  upperArm: number;
  forearm: number;
  /** Épaules → pivot de la tête. */
  neck: number;
}

export class Rig {
  readonly root = new THREE.Group();
  /** Os du squelette, indexés par `J` (le modèle entier est un SkinnedMesh à skinning rigide). */
  readonly joints: THREE.Bone[] = [];
  readonly skeleton: THREE.Skeleton;
  private readonly bodyBase = new THREE.Vector3();

  constructor(readonly spec: RigSpec) {
    const make = (name: string): THREE.Bone => {
      const o = new THREE.Bone();
      o.name = name;
      return o;
    };
    for (let i = 0; i < JOINT_COUNT; i++) this.joints.push(make(`j${i}`));
    const j = (k: keyof typeof J): THREE.Object3D => this.joints[J[k]]!;
    const s = spec;

    this.root.add(j('body')); // pivot au sol : les chutes basculent autour des pieds
    j('body').add(j('hips'));
    j('hips').position.y = s.hipY;
    j('hips').add(j('spine'), j('legL'), j('legR'));
    j('legL').position.z = -s.hipW;
    j('legR').position.z = s.hipW;
    j('legL').add(j('shinL'));
    j('legR').add(j('shinR'));
    j('shinL').position.y = -s.thigh;
    j('shinR').position.y = -s.thigh;
    j('spine').add(j('head'), j('armL'), j('armR'));
    j('head').position.y = s.spine + s.neck;
    j('armL').position.set(0, s.spine, -s.shoulderW);
    j('armR').position.set(0, s.spine, s.shoulderW);
    j('armL').add(j('foreL'));
    j('armR').add(j('foreR'));
    j('foreL').position.y = -s.upperArm;
    j('foreR').position.y = -s.upperArm;
    j('foreR').add(j('prop'));
    j('prop').position.y = -s.forearm;
    this.bodyBase.copy(j('body').position);
    this.root.updateMatrixWorld(true);
    this.skeleton = new THREE.Skeleton(this.joints);
  }

  joint(name: keyof typeof J): THREE.Bone {
    return this.joints[J[name]]!;
  }

  /** Hauteur approximative du sommet de la tête (barres de vie, FX au-dessus). */
  get height(): number {
    return this.spec.hipY + this.spec.spine + this.spec.neck * 2;
  }

  apply(p: Pose): void {
    for (let i = 0; i < JOINT_COUNT; i++) this.joints[i]!.rotation.set(p[i * 3]!, p[i * 3 + 1]!, p[i * 3 + 2]!);
    this.joints[J.body]!.position.set(
      this.bodyBase.x + p[BODY_OFFSET]!,
      this.bodyBase.y + p[BODY_OFFSET + 1]!,
      this.bodyBase.z + p[BODY_OFFSET + 2]!,
    );
  }
}
