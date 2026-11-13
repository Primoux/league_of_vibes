/**
 * Modèles de champions (claude-fx). `installChampionModels()` enregistre la fabrique auprès du renderer
 * (`registerChampionFactory`) et route les événements du fxBus vers les vues (frappes, incantations, rappel).
 */
import type { EntitySnapshot, GameEvent } from '@lov/shared';
import { CHAMPIONS } from '@lov/shared';
import { fxBus } from '../../fx/FxBus.ts';
import { registerChampionFactory } from '../EntityView.ts';
import { ProceduralChampionView, TEAM_HEX, type ChampionModelDef } from './ChampionView.ts';
import { genericFor } from './models/generic.ts';
import { grondakModel } from './models/grondak.ts';
import { ilyraModel } from './models/ilyra.ts';

export { ProceduralChampionView } from './ChampionView.ts';

const MODELS: Record<string, ChampionModelDef> = {
  grondak: grondakModel,
  ilyra: ilyraModel,
};

/** Vues vivantes, par id d'entité. */
const views = new Map<number, ProceduralChampionView>();

export function modelFor(championId: string): ChampionModelDef {
  return MODELS[championId] ?? genericFor(CHAMPIONS[championId]?.class ?? 'fighter');
}

/** Crée la vue d'un champion (null si le championId est inconnu → vue par défaut du renderer). */
export function createChampionView(snap: Pick<EntitySnapshot, 'id' | 'subtype' | 'team'>): ProceduralChampionView | null {
  const id = snap.subtype;
  if (!id || !CHAMPIONS[id]) return null;
  const team = snap.team in TEAM_HEX ? (snap.team as keyof typeof TEAM_HEX) : 'neutral';
  const view = new ProceduralChampionView(snap.id, id, team, modelFor(id));
  views.set(snap.id, view);
  const dispose = view.dispose.bind(view);
  view.dispose = () => {
    if (views.get(snap.id) === view) views.delete(snap.id);
    dispose();
  };
  return view;
}

export function getChampionView(id: number): ProceduralChampionView | undefined {
  return views.get(id);
}

function route(e: GameEvent): void {
  switch (e.type) {
    case 'autoAttack':
      views.get(e.source)?.strike();
      break;
    case 'abilityCast':
      views.get(e.source)?.cast(e.slot);
      break;
    case 'recallStart':
      views.get(e.target)?.setRecalling(true);
      break;
    case 'recallEnd':
      views.get(e.target)?.setRecalling(false);
      break;
  }
}

let installed = false;

export function installChampionModels(): void {
  if (installed) return;
  installed = true;
  registerChampionFactory(createChampionView);
  fxBus.subscribe(route);
}
