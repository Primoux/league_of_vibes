import type { ChampionDef } from '../../schema.ts';
import corvin from './corvin.ts';
import grondak from './grondak.ts';
import ilyra from './ilyra.ts';
import kaelen from './kaelen.ts';
import nyx from './nyx.ts';
import melenchon from './melenchon.ts';
import master67 from './master67.ts';
import touilleur from './touilleur.ts';
import sable from './sable.ts';
import solenne from './solenne.ts';
import tharox from './tharox.ts';
import vrenna from './vrenna.ts';
import zephyr from './zephyr.ts';

export const CHAMPION_LIST: ChampionDef[] = [grondak, tharox, kaelen, vrenna, ilyra, zephyr, sable, corvin, nyx, solenne, melenchon, master67, touilleur];
export const CHAMPIONS: Record<string, ChampionDef> = Object.fromEntries(CHAMPION_LIST.map((c) => [c.id, c]));
export const getChampion = (id: string): ChampionDef => {
  const c = CHAMPIONS[id];
  if (!c) throw new Error(`Champion inconnu: ${id}`);
  return c;
};
