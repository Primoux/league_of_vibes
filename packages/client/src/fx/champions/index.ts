/**
 * Enregistrement des FX dédiés des champions (effet de bord à l'import : `registerFx`).
 * Les champions absents d'ici utilisent les FX génériques (`defaults.ts`).
 */
import './grondak.ts';
import './master67.ts';
import './touilleur.ts';
import './ilyra.ts';
import './tharox.ts';
import './kaelen.ts';
import './vrenna.ts';
import './zephyr.ts';
import './sable.ts';
import './corvin.ts';
import './nyx.ts';
import './solenne.ts';
import { installMelenchonFx } from './melenchon.ts';
installMelenchonFx();
