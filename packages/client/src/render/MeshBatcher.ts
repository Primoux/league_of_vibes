/**
 * Regroupement des meshes en InstancedMesh. Propriétaire : claude-client.
 *
 * Les vues procédurales (sbires, monstres, structures) sont des hiérarchies d'Object3D animées par pivots.
 * `adopt(root)` remplace chaque Mesh de la hiérarchie par un nœud vide (même transform, mêmes enfants) et
 * l'inscrit dans un lot partagé par (géométrie, matériau, ombre). À chaque frame, `sync()` recopie la
 * matrixWorld des nœuds visibles dans l'InstancedMesh du lot : un draw call par lot au lieu d'un par pièce.
 *
 * Les animations continuent de manipuler les pivots comme avant. Aucune allocation par frame.
 */
import * as THREE from 'three';

interface Batch {
  mesh: THREE.InstancedMesh;
  nodes: BatchNode[];
  capacity: number;
}

interface BatchNode {
  node: THREE.Object3D;
  /** Racine de la vue (sa visibilité masque tous ses nœuds). */
  root: THREE.Object3D;
  batch: Batch;
}

export interface BatchHandle {
  nodes: BatchNode[];
}

const INITIAL_CAPACITY = 16;

/** Visible si le nœud et tous ses ancêtres jusqu'à la racine de la vue (incluse) sont visibles. */
function chainVisible(node: THREE.Object3D, root: THREE.Object3D): boolean {
  let o: THREE.Object3D | null = node;
  while (o) {
    if (!o.visible) return false;
    if (o === root) return !!root.parent;
    o = o.parent;
  }
  return false;
}

export class MeshBatcher {
  private batches = new Map<string, Batch>();
  private geoIds = new WeakMap<THREE.BufferGeometry, number>();
  private nextGeoId = 1;

  constructor(private readonly scene: THREE.Scene) {}

  get batchCount(): number {
    return this.batches.size;
  }

  private geoId(g: THREE.BufferGeometry): number {
    let id = this.geoIds.get(g);
    if (!id) {
      id = this.nextGeoId++;
      this.geoIds.set(g, id);
    }
    return id;
  }

  private batchFor(geo: THREE.BufferGeometry, mat: THREE.Material, cast: boolean, receive: boolean): Batch {
    const key = `${this.geoId(geo)}|${mat.uuid}|${cast ? 1 : 0}${receive ? 1 : 0}`;
    let b = this.batches.get(key);
    if (!b) {
      b = { mesh: this.makeMesh(geo, mat, INITIAL_CAPACITY, cast, receive), nodes: [], capacity: INITIAL_CAPACITY };
      this.batches.set(key, b);
    }
    return b;
  }

  private makeMesh(geo: THREE.BufferGeometry, mat: THREE.Material, capacity: number, cast: boolean, receive: boolean): THREE.InstancedMesh {
    const m = new THREE.InstancedMesh(geo, mat, capacity);
    m.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    m.count = 0;
    m.castShadow = cast;
    m.receiveShadow = receive;
    // les instances couvrent toute la carte : pas de culling global (le GPU rejette les sommets hors champ)
    m.frustumCulled = false;
    this.scene.add(m);
    return m;
  }

  private grow(b: Batch): void {
    const old = b.mesh;
    b.capacity *= 2;
    b.mesh = this.makeMesh(old.geometry, old.material as THREE.Material, b.capacity, old.castShadow, old.receiveShadow);
    old.removeFromParent();
    old.dispose();
  }

  /**
   * Remplace les Mesh de `root` par des nœuds batchés. Les InstancedMesh, SkinnedMesh, Points, Lines, Sprites
   * et matériaux multiples sont laissés tels quels.
   */
  adopt(root: THREE.Object3D): BatchHandle {
    const meshes: THREE.Mesh[] = [];
    root.traverse((o) => {
      const m = o as THREE.Mesh;
      if (m.isMesh && !(o as THREE.InstancedMesh).isInstancedMesh && !(o as THREE.SkinnedMesh).isSkinnedMesh && !Array.isArray(m.material)) meshes.push(m);
    });
    const handle: BatchHandle = { nodes: [] };
    for (const m of meshes) {
      const node = new THREE.Object3D();
      node.name = m.name;
      node.position.copy(m.position);
      node.quaternion.copy(m.quaternion);
      node.scale.copy(m.scale);
      node.visible = m.visible;
      const parent = m.parent;
      if (parent) {
        parent.add(node);
        parent.remove(m);
      }
      // les enfants du mesh (rare) passent sous le nœud
      while (m.children.length) node.add(m.children[0]!);
      const batch = this.batchFor(m.geometry, m.material as THREE.Material, m.castShadow, m.receiveShadow);
      const entry: BatchNode = { node, root, batch };
      batch.nodes.push(entry);
      handle.nodes.push(entry);
    }
    return handle;
  }

  release(handle: BatchHandle): void {
    for (const entry of handle.nodes) {
      const arr = entry.batch.nodes;
      const i = arr.indexOf(entry);
      if (i >= 0) {
        arr[i] = arr[arr.length - 1]!;
        arr.pop();
      }
    }
    handle.nodes.length = 0;
  }

  /** Recopie les matrices monde (à appeler après scene.updateMatrixWorld(), avant le rendu). */
  sync(): void {
    for (const b of this.batches.values()) {
      if (b.nodes.length > b.capacity) {
        while (b.nodes.length > b.capacity) b.capacity *= 2;
        b.capacity /= 2;
        this.grow(b);
      }
      let n = 0;
      const arr = b.mesh.instanceMatrix.array as Float32Array;
      for (const entry of b.nodes) {
        if (!chainVisible(entry.node, entry.root)) continue;
        entry.node.matrixWorld.toArray(arr, n * 16);
        n++;
      }
      b.mesh.count = n;
      b.mesh.instanceMatrix.needsUpdate = true;
      b.mesh.visible = n > 0;
    }
  }
}
