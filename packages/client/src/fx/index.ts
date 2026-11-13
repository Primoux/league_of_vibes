/**
 * Point d'entrée FX. Propriétaire : claude-fx.
 * BOOTSTRAP : stub. installFx() reçoit la scène et s'abonne au fxBus pour jouer les effets par fxKey.
 */
import type * as THREE from 'three';
import { fxBus } from './FxBus.ts';

export function installFx(_scene: THREE.Scene): void {
  fxBus.subscribe(() => {
    /* TODO(claude-fx) : particules / shaders par événement */
  });
}
