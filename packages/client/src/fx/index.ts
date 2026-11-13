/**
 * Point d'entrée FX. Propriétaire : claude-fx.
 *
 * `installFx(host)` crée le runtime FX dans la scène de l'hôte (GameRenderer ou fxlab), s'abonne au fxBus,
 * fait tourner la boucle FX et, si l'hôte le permet (`setRenderPipeline`), branche le post-process.
 *
 * API publique pour les autres agents client :
 *  - `fxIndicators` : indicateurs de visée/portée (claude-client) ;
 *  - `setFxQuality('high' | 'low')` : qualité (store/paramètres) ;
 *  - `registerFx` / `onWorldEvent` : extension des FX.
 */
import { installChampionModels, setRespawnHook } from '../render/champions/index.ts';
import { fxBus } from './FxBus.ts';
import { FxRuntime } from './core/FxRuntime.ts';
import type { FxHost } from './core/host.ts';
import { defaultFx } from './defaults.ts';
import { FxDispatcher } from './dispatcher.ts';
import { PostFx, type FxQuality } from './post/PostFx.ts';
import { fxIndicators } from './telegraph/indicators.ts';
import { installAutoAttackFx } from './world/autoAttacks.ts';
import { installStatusFx } from './world/status.ts';
import { installWorldFx, respawnFx } from './world/events.ts';
import './summoners.ts';
import './champions/index.ts';

let worldFxInstalled = false;

export { fxBus } from './FxBus.ts';
export { fxIndicators } from './telegraph/indicators.ts';
export { registerFx, onWorldEvent } from './registry.ts';
export type { FxHost } from './core/host.ts';
export type { FxQuality } from './post/PostFx.ts';

export interface FxInstance {
  runtime: FxRuntime;
  /** Temps CPU des FX par frame (ms, moyenne glissante) : bus + effets + particules. */
  readonly stats: { updateMs: number };
  dispatcher: FxDispatcher;
  post: PostFx | null;
  /** Avance les FX (appelé automatiquement sauf si `manualUpdate`). */
  update(dt: number): void;
  dispose(): void;
}

export interface InstallOptions {
  /** L'appelant appelle lui-même `update(dt)` (fxlab). */
  manualUpdate?: boolean;
  /** Forcer la création du post-process même sans `setRenderPipeline` (l'appelant appelle `post.render()`). */
  ownPost?: boolean;
}

let current: FxInstance | null = null;

export function installFx(host: FxHost, opts: InstallOptions = {}): FxInstance {
  current?.dispose();
  installChampionModels();
  if (!worldFxInstalled) {
    worldFxInstalled = true;
    installAutoAttackFx();
    installStatusFx();
    installWorldFx();
  }
  const runtime = new FxRuntime(host);
  const dispatcher = new FxDispatcher(runtime, defaultFx);
  setRespawnHook((id) => respawnFx(runtime, id));
  fxIndicators.attach(runtime.root);
  const unsub = fxBus.subscribe((e) => dispatcher.handle(e));

  let post: PostFx | null = null;
  if (host.renderer && (host.setRenderPipeline || opts.ownPost)) {
    post = new PostFx(host.renderer, host.scene, host.camera);
    if (!opts.ownPost) host.setRenderPipeline?.(() => post!.render());
  }
  const onResize = (): void => post?.setSize(innerWidth, innerHeight);
  if (post && !opts.ownPost) addEventListener('resize', onResize);

  const stats = { updateMs: 0 };
  const update = (dt: number): void => {
    const t0 = performance.now();
    fxBus.update();
    runtime.update(Math.min(dt, 0.1));
    stats.updateMs += (performance.now() - t0 - stats.updateMs) * 0.05;
  };

  let raf = 0;
  if (!opts.manualUpdate) {
    if (host.onFrame) host.onFrame(update);
    else {
      let last = performance.now();
      const loop = (now: number): void => {
        update((now - last) / 1000);
        last = now;
        raf = requestAnimationFrame(loop);
      };
      raf = requestAnimationFrame(loop);
    }
  }

  current = {
    runtime,
    stats,
    dispatcher,
    post,
    update,
    dispose() {
      unsub();
      cancelAnimationFrame(raf);
      removeEventListener('resize', onResize);
      post?.dispose();
      setRespawnHook(null);
      runtime.entities.dispose();
      host.scene.remove(runtime.root);
      if (current === this) current = null;
    },
  };
  return current;
}

/** Qualité des FX : 'low' = sans post-process et moitié moins de particules. */
export function setFxQuality(q: FxQuality): void {
  if (!current) return;
  if (current.post) current.post.quality = q;
  current.runtime.ps.density = q === 'low' ? 0.5 : 1;
}
