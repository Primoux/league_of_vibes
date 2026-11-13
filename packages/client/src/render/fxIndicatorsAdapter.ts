/**
 * Adaptateur : affiche l'état de visée (`AimState`) avec les indicateurs de claude-fx (`fxIndicators`).
 * Propriétaire : claude-client. Enregistré dans main.ts via `registerIndicatorRenderer`.
 */
import type { AbilityDef, SummonerDef, Vec2 } from '@lov/shared';
import type { AimState, IndicatorRenderer } from './Indicators.ts';

/** Sous-ensemble de l'API `fxIndicators` (client/src/fx/telegraph/indicators.ts) utilisé ici. */
export interface FxIndicatorsApi {
  showAbility(def: AbilityDef, caster: Vec2, cursor: Vec2): void;
  showSummoner(def: SummonerDef, caster: Vec2, cursor: Vec2): void;
  showRange(center: Vec2, radius: number): void;
  hideRange(): void;
  showAim(shape: null): void;
  hide(): void;
}

export function fxIndicatorRenderer(fx: FxIndicatorsApi): IndicatorRenderer {
  let shown = false;
  return {
    update(aim: Readonly<AimState>): void {
      if (aim.active && aim.def && (aim.kind === 'ability' || aim.kind === 'summoner')) {
        if (aim.kind === 'ability') fx.showAbility(aim.def as AbilityDef, aim.origin, aim.cursor);
        else fx.showSummoner(aim.def as SummonerDef, aim.origin, aim.cursor);
        shown = true;
      } else if (aim.attackRange > 0) {
        fx.showAim(null);
        fx.showRange(aim.origin, aim.attackRange);
        shown = true;
      } else if (shown) {
        fx.hide();
        shown = false;
      }
    },
  };
}
