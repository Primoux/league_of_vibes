/** Caricature originale, sans texture : costume, lunettes et gestuelle de tribun. */
import * as THREE from 'three';
import type { EntitySnapshot } from '@lov/shared';
import type { EntityView } from '../EntityView.ts';

export class MelenchonView implements EntityView {
  readonly object3d = new THREE.Group();
  private readonly rig = new THREE.Group();
  private readonly torso = new THREE.Group();
  private readonly head = new THREE.Group();
  private readonly arms = [new THREE.Group(), new THREE.Group()];
  private readonly legs = [new THREE.Group(), new THREE.Group()];
  private readonly republic = new THREE.Group();
  private readonly mouth: THREE.Mesh;
  private readonly tie: THREE.Mesh;
  private readonly geometries = new Map<string, THREE.BufferGeometry>();
  private readonly materials: THREE.MeshStandardMaterial[] = [];
  private anim = 'idle';
  private age = 0;
  private time = 0;
  private deathTime = 0;
  private recalling = false;
  private disposed = false;

  constructor(readonly hologram = false, team: EntitySnapshot['team'] = 'blue') {
    this.object3d.name = hologram ? 'melenchon-hologramme' : 'melenchon';
    this.rig.rotation.y = Math.PI / 2; // Le visage regarde +x, comme les autres champions.
    this.object3d.add(this.rig);
    const material = (color: number, metalness = 0): THREE.MeshStandardMaterial => {
      const m = new THREE.MeshStandardMaterial({ color: hologram ? 0x55ddff : color,
        roughness: metalness ? 0.3 : 0.72, metalness,
        transparent: hologram, opacity: hologram ? 0.32 : 1,
        depthWrite: !hologram, emissive: hologram ? 0x167b9c : 0, emissiveIntensity: 0.7 });
      this.materials.push(m);
      return m;
    };
    const navy = material(0x20304a), lapel = material(0x304566), trousers = material(0x192638);
    const skin = material(0xdcae93), warm = material(0xc28b75), white = material(0xf1eee6);
    const red = material(0xce263b), hair = material(0xaaa9a5), hairLight = material(0xc2c1bc), silver = material(0xd5d3cb, 0.65);
    const dark = material(0x302e30), black = material(0x18202a), blue = material(0x195dce);
    const teamMat = material(team === 'red' ? 0xff4a4a : team === 'blue' ? 0x3a7bff : 0xc9a227);
    const geo = (kind: 'sphere' | 'box' | 'cylinder' | 'torus'): THREE.BufferGeometry => {
      let g = this.geometries.get(kind);
      if (!g) {
        g = kind === 'sphere' ? new THREE.SphereGeometry(1, 16, 12)
          : kind === 'box' ? new THREE.BoxGeometry(1, 1, 1)
          : kind === 'cylinder' ? new THREE.CylinderGeometry(1, 1, 1, 12)
          : new THREE.TorusGeometry(1, 0.026, 4, 20);
        this.geometries.set(kind, g);
      }
      return g;
    };
    const mesh = (parent: THREE.Object3D, name: string, kind: Parameters<typeof geo>[0], mat: THREE.Material,
      pos: number[], scale: number[]): THREE.Mesh => {
      const m = new THREE.Mesh(geo(kind), mat);
      m.name = name;
      m.position.set(pos[0]!, pos[1]!, pos[2]!);
      m.scale.set(scale[0]!, scale[1]!, scale[2]!);
      m.castShadow = !hologram; m.receiveShadow = !hologram;
      parent.add(m);
      return m;
    };
    this.rig.add(this.torso, ...this.legs, this.republic);
    const teamRing = mesh(this.rig, 'anneau-equipe', 'torus', teamMat, [0, 0.035, 0], [0.57, 0.57, 0.57]);
    teamRing.rotation.x = Math.PI / 2;
    this.torso.position.y = 0.92;
    mesh(this.torso, 'veste-croisee', 'sphere', navy, [0, 0.36, 0], [0.42, 0.58, 0.26]);
    mesh(this.torso, 'chemise', 'box', white, [0, 0.65, 0.242], [0.25, 0.46, 0.04]);
    for (const side of [-1, 1]) {
      const l = mesh(this.torso, 'revers', 'box', lapel, [side * 0.17, 0.61, 0.254], [0.12, 0.45, 0.045]);
      l.rotation.z = side * -0.3;
      const collar = mesh(this.torso, 'col', 'box', white, [side * 0.066, 0.84, 0.25], [0.105, 0.13, 0.035]);
      collar.rotation.z = side * 0.45;
    }
    mesh(this.torso, 'noeud-cravate', 'sphere', red, [0, 0.78, 0.29], [0.055, 0.054, 0.027]);
    this.tie = mesh(this.torso, 'cravate-rouge', 'box', red, [0, 0.53, 0.288], [0.085, 0.4, 0.025]);
    this.tie.rotation.z = -0.05;
    mesh(this.torso, 'bouton', 'sphere', silver, [0.04, 0.23, 0.26], [0.018, 0.018, 0.012]);
    mesh(this.torso, 'pochette', 'box', white, [-0.28, 0.69, 0.245], [0.08, 0.025, 0.018]);
    mesh(this.torso, 'cou', 'cylinder', skin, [0, 0.98, 0], [0.13, 0.2, 0.12]);
    this.head.position.set(0, 1.28, 0.015);
    this.torso.add(this.head);
    // Visage large, joues basses, front haut et nez proéminent : lisible même en vue de jeu.
    mesh(this.head, 'visage', 'sphere', skin, [0, 0, 0], [0.32, 0.4, 0.265]);
    mesh(this.head, 'machoire', 'sphere', skin, [0, -0.21, 0.045], [0.29, 0.19, 0.24]);
    for (const side of [-1, 1]) {
      mesh(this.head, 'oreille', 'sphere', warm, [side * 0.322, -0.025, 0], [0.06, 0.11, 0.055]);
      mesh(this.head, 'joue', 'sphere', skin, [side * 0.18, -0.11, 0.13], [0.125, 0.12, 0.11]);
      mesh(this.head, 'oeil', 'sphere', white, [side * 0.139, 0.038, 0.25], [0.071, 0.029, 0.015]);
      mesh(this.head, 'regard', 'sphere', dark, [side * 0.133, 0.039, 0.267], [0.018, 0.02, 0.008]);
      const brow = mesh(this.head, 'sourcil-indigne', 'box', dark, [side * 0.139, 0.117, 0.252], [0.16, 0.027, 0.024]);
      brow.rotation.z = side * 0.13;
      // Quatre branches par verre : silhouette rectangulaire plutôt que lunettes rondes génériques.
      for (const y of [-0.016, 0.1]) mesh(this.head, 'lunettes-monture', 'box', silver, [side * 0.145, y, 0.285], [0.22, 0.012, 0.014]);
      for (const x of [0.035, 0.255]) mesh(this.head, 'lunettes-monture', 'box', silver, [side * x, 0.042, 0.285], [0.012, 0.116, 0.014]);
      mesh(this.head, 'lunettes-branche', 'box', silver, [side * 0.28, 0.075, 0.13], [0.012, 0.015, 0.3]);
      const fold = mesh(this.head, 'pli-nasolabial', 'box', warm, [side * 0.108, -0.165, 0.268], [0.011, 0.1, 0.008]);
      fold.rotation.z = side * 0.3;
      mesh(this.head, 'tempe-grise', 'sphere', hair, [side * 0.283, 0.15, -0.035], [0.065, 0.19, 0.2]);
    }
    mesh(this.head, 'lunettes-pont', 'box', silver, [0, 0.055, 0.299], [0.06, 0.013, 0.014]);
    const nose = mesh(this.head, 'nez', 'sphere', skin, [0, -0.035, 0.294], [0.066, 0.13, 0.105]);
    nose.rotation.x = -0.18;
    mesh(this.head, 'pointe-nez', 'sphere', warm, [0, -0.112, 0.333], [0.073, 0.043, 0.058]);
    this.mouth = mesh(this.head, 'bouche', 'sphere', dark, [0, -0.238, 0.273], [0.11, 0.016, 0.012]);
    mesh(this.head, 'levre', 'sphere', warm, [0, -0.263, 0.268], [0.1, 0.019, 0.016]);
    // Masse de cheveux rejetée en arrière et mèches séparées (front dégagé).
    mesh(this.head, 'cheveux-arriere', 'sphere', hair, [0, 0.22, -0.1], [0.32, 0.205, 0.24]);
    for (let i = 0; i < 7; i++) {
      const lock = mesh(this.head, 'meche-coiffee-en-arriere', 'sphere', i % 2 ? hairLight : hair,
        [(i - 3) * 0.078, 0.32 + (1 - Math.abs(i - 3) / 4) * 0.035, -0.045], [0.052, 0.045, 0.23]);
      lock.rotation.x = -0.18; lock.rotation.z = 0.17;
    }
    for (let i = 0; i < 2; i++) {
      const side = i === 0 ? -1 : 1;
      const leg = this.legs[i]!; leg.position.set(side * 0.18, 0.88, 0);
      mesh(leg, 'pantalon', 'cylinder', trousers, [0, -0.38, 0], [0.135, 0.75, 0.14]);
      mesh(leg, 'chaussure', 'sphere', black, [0, -0.8, 0.065], [0.145, 0.1, 0.24]);
      const arm = this.arms[i]!; arm.position.set(side * 0.42, 0.77, 0);
      this.torso.add(arm);
      mesh(arm, 'epaule', 'sphere', navy, [0, 0, 0], [0.12, 0.13, 0.12]);
      mesh(arm, 'manche', 'cylinder', navy, [0, -0.27, 0], [0.12, 0.54, 0.12]);
      mesh(arm, 'poignet', 'cylinder', white, [0, -0.56, 0], [0.1, 0.065, 0.1]);
      mesh(arm, 'main', 'sphere', skin, [0, -0.66, 0.01], [0.105, 0.125, 0.085]);
      if (i === 1) {
        // L'index fait partie de la silhouette ; rotations du bras = doigt accusateur.
        mesh(arm, 'index-accusateur', 'cylinder', skin, [0.046, -0.83, 0.018], [0.028, 0.24, 0.028]);
      } else {
        mesh(arm, 'dossier-parlementaire', 'box', red, [0, -0.67, 0.1], [0.23, 0.3, 0.055]);
        mesh(arm, 'feuilles', 'box', white, [0, -0.67, 0.133], [0.2, 0.27, 0.012]);
      }
    }
    for (let i = 0; i < 3; i++) {
      const ring = mesh(this.republic, 'aureole-republicaine', 'torus', [blue, white, red][i]!, [0, 0.06 + i * 0.07, 0], [0.7 + i * 0.09, 0.7 + i * 0.09, 0.7 + i * 0.09]);
      ring.rotation.x = Math.PI / 2;
    }
    this.republic.visible = false;
  }

  playAnim(name: string): void {
    // Un snapshot idle ne coupe pas un geste après seulement un tick serveur.
    if (name === 'idle' && this.anim.startsWith('cast_') && this.age < 0.75) return;
    if (name !== this.anim || name.startsWith('cast_') || name === 'attack') { this.anim = name; this.age = 0; }
  }

  get height(): number { return 2.65; }
  strike(): void { this.playAnim('attack'); }
  cast(slot: string): void { this.playAnim(`cast_${slot}`); }
  setRecalling(on: boolean): void { this.recalling = on; }

  update(snap: EntitySnapshot, dt: number): void {
    if (this.disposed) return;
    this.time += dt; this.age += dt;
    this.deathTime = snap.dead ? this.deathTime + dt : 0;
    this.rig.visible = this.deathTime < 1.4;
    if (snap.dead || this.anim === 'run') this.recalling = false;
    const ult = snap.buffs?.some((b) => b.id === 'melenchon_r') ?? false;
    const declaim = !snap.dead && (ult || (this.anim === 'cast_R' && this.age < 1.3));
    const point = !snap.dead && ((this.anim === 'cast_Q' || this.anim === 'attack') && this.age < 0.85);
    const run = !snap.dead && this.anim === 'run';
    const gait = run ? Math.sin(this.time * 11) * 0.55 : 0;
    this.legs[0]!.rotation.x = gait; this.legs[1]!.rotation.x = -gait;
    this.arms[0]!.rotation.set(declaim ? -2.45 : -gait * 0.6, 0, declaim ? -0.45 : 0.09);
    this.arms[1]!.rotation.set(declaim ? -2.8 : point ? -1.85 : gait * 0.6, 0, declaim ? 0.35 : -0.1);
    this.head.rotation.set(declaim ? -0.16 : this.recalling ? 0.15 : 0.02 * Math.sin(this.time * 1.5), point ? -0.1 : 0, 0);
    this.torso.position.y = 0.92 + (run ? Math.abs(gait) * 0.05 : Math.sin(this.time * 2) * 0.012);
    this.torso.rotation.x = snap.dead ? -1.2 : this.anim === 'cast_W' && this.age < 0.7 ? 0.15 : 0;
    this.rig.position.y = snap.dead ? -0.6 : 0;
    this.rig.scale.setScalar(ult ? 1.12 : 1);
    this.tie.rotation.x = Math.sin(this.time * (run ? 11 : 2)) * (run ? 0.22 : 0.025);
    this.mouth.scale.y = declaim ? 0.058 + Math.sin(this.time * 16) * 0.012 : 0.016;
    this.republic.visible = ult;
    this.republic.rotation.y = this.time * 0.65;
    if (this.anim.startsWith('cast_') && this.age > 1.4) this.anim = 'idle';
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const g of this.geometries.values()) g.dispose();
    for (const m of this.materials) m.dispose();
    this.object3d.removeFromParent();
    this.object3d.clear();
  }
}
