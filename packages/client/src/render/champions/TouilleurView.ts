import type { EntitySnapshot } from '@lov/shared';
import { ProceduralChampionView, TEAM_HEX } from './ChampionView.ts';
import { touilleurModel } from './models/touilleur.ts';

/** Le canal suit son buff serveur, y compris une fin anticipée par purification ou interruption. */
export class TouilleurView extends ProceduralChampionView {
  private stirring = false;
  constructor(entityId: number, team: keyof typeof TEAM_HEX) { super(entityId, 'touilleur', team, touilleurModel); }

  override update(snap: EntitySnapshot, dt: number): void {
    const stirring = !snap.dead && !!snap.buffs?.some(b => b.id === 'touilleur_r_channel');
    if (stirring && !this.stirring) this.animator.play('cast_R', { fade: .06 });
    else if (!stirring && this.stirring && this.animator.current === 'cast_R') this.animator.play('idle', { fade: .1 });
    this.stirring = stirring;
    super.update(snap, dt);
  }
}
