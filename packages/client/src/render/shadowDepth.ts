/**
 * Matériaux de profondeur d'ombre dédiés par type d'objet. Propriétaire : claude-client (optimisation claude-fx).
 *
 * three dessine la passe d'ombre avec UN `MeshDepthMaterial` partagé par tous les objets. Ce matériau ne peut
 * avoir qu'un programme à la fois (avec ou sans instancing, skinning, couleur d'instance) : alterner des meshes
 * de types différents le force à changer de programme à chaque bascule (`getProgram` → `getParameters` à
 * chaque frame). Un matériau de profondeur partagé **par type** garde chaque programme stable.
 */
import * as THREE from 'three';

const instanced = new THREE.MeshDepthMaterial();
const instancedColor = new THREE.MeshDepthMaterial();
const skinned = new THREE.MeshDepthMaterial();

/** Matériau de profondeur adapté à `o` (undefined pour un mesh simple : celui de three convient). */
export function shadowDepthFor(o: THREE.Object3D): THREE.Material | undefined {
  if ((o as THREE.InstancedMesh).isInstancedMesh) return (o as THREE.InstancedMesh).instanceColor ? instancedColor : instanced;
  if ((o as THREE.SkinnedMesh).isSkinnedMesh) return skinned;
  return undefined;
}

/** Affecte le matériau de profondeur dédié à un objet qui projette une ombre (sans écraser un matériau existant). */
export function assignShadowDepth(o: THREE.Object3D): void {
  if (!o.castShadow || o.customDepthMaterial) return;
  const m = shadowDepthFor(o);
  if (m) o.customDepthMaterial = m;
}

/** Idem pour tout un sous-arbre (à appeler une fois à la construction). */
export function assignShadowDepthTree(root: THREE.Object3D): void {
  root.traverse(assignShadowDepth);
}
