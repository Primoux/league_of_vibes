/**
 * Modèles de champions (claude-fx). `installChampionModels()` enregistre la fabrique auprès du renderer
 * (`registerChampionFactory`) et route les événements du fxBus vers les vues (frappes, incantations, rappel).
 */
import type { EntitySnapshot, GameEvent } from '@lov/shared';
import { CHAMPIONS } from '@lov/shared';
import { fxBus } from '../../fx/FxBus.ts';
import { registerChampionFactory } from '../EntityView.ts';
import type { EntityView } from '../EntityView.ts';
import { MelenchonView } from './melenchon.ts';
import { ProceduralChampionView, TEAM_HEX, type ChampionModelDef } from './ChampionView.ts';
import { genericFor } from './models/generic.ts';
import { grondakModel } from './models/grondak.ts';
import { ilyraModel } from './models/ilyra.ts';
import { corvinModel } from './models/corvin.ts';
import { kaelenModel } from './models/kaelen.ts';
import { nyxModel } from './models/nyx.ts';
import { sableModel } from './models/sable.ts';
import { solenneModel } from './models/solenne.ts';
import { tharoxModel } from './models/tharox.ts';
import { vrennaModel } from './models/vrenna.ts';
import { zephyrModel } from './models/zephyr.ts';

export { ProceduralChampionView, setRespawnHook } from './ChampionView.ts';

const MODELS: Record<string, ChampionModelDef> = {
  grondak: grondakModel,
  ilyra: ilyraModel,
  tharox: tharoxModel,
  kaelen: kaelenModel,
  vrenna: vrennaModel,
  zephyr: zephyrModel,
  sable: sableModel,
  corvin: corvinModel,
  nyx: nyxModel,
  solenne: solenneModel,
};

/** Contrat commun aux modèles articulés et aux caricatures originales. */
export interface AnimatedChampionView extends EntityView {
  readonly height: number;
  strike(): void;
  cast(slot: string): void;
  setRecalling(on: boolean): void;
}

/** Vues vivantes, par id d'entité. */
const views = new Map<number, AnimatedChampionView>();

export function modelFor(championId: string): ChampionModelDef {
  return MODELS[championId] ?? genericFor(CHAMPIONS[championId]?.class ?? 'fighter');
}

/** Crée la vue d'un champion (null si le championId est inconnu → vue par défaut du renderer). */
export function createChampionView(snap: Pick<EntitySnapshot, 'id' | 'subtype' | 'team'>): AnimatedChampionView | null {
  const id = snap.subtype;
  if (!id || !CHAMPIONS[id]) return null;
  const team = snap.team in TEAM_HEX ? (snap.team as keyof typeof TEAM_HEX) : 'neutral';
  const view = id === 'melenchon' ? new MelenchonView(false, team) : new ProceduralChampionView(snap.id, id, team, modelFor(id));
  views.set(snap.id, view);
  const dispose = view.dispose.bind(view);
  view.dispose = () => {
    if (views.get(snap.id) === view) views.delete(snap.id);
    dispose();
  };
  return view;
}

export function getChampionView(id: number): AnimatedChampionView | undefined {
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
