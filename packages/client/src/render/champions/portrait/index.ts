import type { ChampionModelDef } from '../ChampionView.ts';
import { Sculpt } from './geometry.ts';
import { humanoidPortraits } from './humanoids.ts';
import { creaturePortraits } from './creatures.ts';

const builds = { ...humanoidPortraits, ...creaturePortraits };
const cache = new WeakMap<ChampionModelDef, ChampionModelDef>();

/** La géométrie change ; rig, clips, effets, port d'arme et gameplay restent ceux du champion. */
export function portraitModel(id: string, base: ChampionModelDef): ChampionModelDef {
  const build = builds[id];
  if (!build) return base;
  let model = cache.get(base);
  if (!model) {
    model = { ...base, build: (builder, colors) => build(new Sculpt(builder), colors, base.rig) };
    cache.set(base, model);
  }
  return model;
}
