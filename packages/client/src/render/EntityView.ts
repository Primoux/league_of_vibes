/**
 * Interface des vues d'entités 3D et registre de fabriques de champions. Propriétaire : claude-client.
 *
 * Contrat avec claude-fx (`render/champions/**`) :
 *   registerChampionFactory((snap) => ChampionView)
 * Le renderer place `object3d` (position au sol y = 0, rotation.y = orientation) à chaque frame à partir
 * de la position interpolée ; la vue gère son contenu (modèle, animations) dans `update` et `playAnim`.
 * Un objet orienté vers +x local regarde dans la direction `facing` de l'entité.
 */
import type * as THREE from 'three';
import type { EntitySnapshot } from '@lov/shared';

export interface EntityView {
  readonly object3d: THREE.Object3D;
  /** Appelé chaque frame avec le dernier état connu de l'entité. `dt` en secondes. */
  update(snap: EntitySnapshot, dt: number): void;
  /** Animation demandée (snap.anim : idle/run/attack/cast_Q/death/recall…), appelée quand elle change. */
  playAnim(name: string): void;
  dispose(): void;
}

export type ChampionView = EntityView;
export type ChampionFactory = (snap: EntitySnapshot) => ChampionView | null;

let championFactory: ChampionFactory | null = null;

/** claude-fx enregistre ici la fabrique des modèles de champions. Retourner null = vue par défaut. */
export function registerChampionFactory(factory: ChampionFactory): void {
  championFactory = factory;
}

export function getChampionFactory(): ChampionFactory | null {
  return championFactory;
}
