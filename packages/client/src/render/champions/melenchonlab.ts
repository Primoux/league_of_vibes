/** Vitrine visuelle autonome : ?melenchonlab (les règles réelles restent côté serveur). */
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { getChampion, type AbilitySlot, type EntitySnapshot, type GameEvent } from '@lov/shared';
import { installMelenchonFx } from '../../fx/champions/melenchon.ts';
import { installFx } from '../../fx/index.ts';
import { MelenchonView } from './melenchon.ts';

export function startMelenchonLab(canvas: HTMLCanvasElement): void {
  document.title = 'Mélenchon · Le Tribun indigné — League of Vibes';
  const def = getChampion('melenchon');
  const style = document.createElement('style');
  style.textContent = `
    body{margin:0;background:#0c1525;color:#f7efdf;font-family:Arial,sans-serif;overflow:hidden}
    #ui{display:none} .tribun{position:fixed;inset:0;display:grid;grid-template-columns:360px 1fr}
    .tribun-panel{padding:32px 28px;background:#111e32;overflow:auto;border-right:1px solid #ffffff16;z-index:2}
    .tribun-brand{font-size:10px;font-weight:700;letter-spacing:3px;color:#a6bad3;margin:0 0 36px}
    .tribun-tag{color:#ef6570;font-size:10px;letter-spacing:2px;font-weight:bold}
    .tribun h1{font:58px/.98 Georgia,serif;letter-spacing:-2px;margin:12px 0 16px}
    .tribun-intro{color:#a9b9cf;font-size:13px;line-height:1.7;margin:0 0 24px}
    .tribun-passive{border-left:2px solid #e34b58;padding-left:12px;margin:20px 0 24px;font-size:12px;line-height:1.6;color:#a9b9cf}
    .tribun-passive strong{color:#f7efdf}.tribun-skills{display:grid;gap:8px}
    .tribun button{font:inherit;cursor:pointer;color:inherit}
    .tribun-skill{display:flex;text-align:left;align-items:center;gap:12px;background:#ffffff05;border:1px solid #ffffff17;border-radius:6px;padding:12px;transition:background .15s}
    .tribun-skill:hover,.tribun-skill[aria-pressed=true]{background:#263c5c;border-color:#7991b2}
    .tribun-skill kbd{display:grid;place-items:center;flex:0 0 30px;height:30px;background:#ffffff09;border:1px solid #ffffff28;border-radius:4px;font:16px Georgia}
    .tribun-skill strong{font-size:12px;display:block}.tribun-skill small{display:block;color:#a9b9cf;font-size:10px;margin-top:5px}
    .tribun-skill:last-child{border-color:#a23b4a}.tribun-foot{font-size:10px;line-height:1.7;color:#7488a3;margin-top:22px}
    .tribun-stage{position:relative;min-width:0;background:radial-gradient(ellipse at 50% 40%,#263c58,#0b1423 75%)}
    .tribun-stage canvas{position:absolute!important;inset:0!important;width:100%!important;height:100%!important}
    .tribun-tools{position:absolute;top:24px;right:26px;display:flex;gap:7px;z-index:1}
    .tribun-tools button{background:#14223acc;border:1px solid #ffffff28;border-radius:30px;font-size:11px;padding:9px 14px}
    .tribun-caption{position:absolute;bottom:30px;left:32px;right:32px;pointer-events:none}
    .tribun-caption span{font-size:10px;letter-spacing:3px;color:#8aa0be}.tribun-caption p{font:22px Georgia;margin:9px 0;color:#f7efdf}
    .tribun-status{position:absolute;top:28px;left:30px;font-size:10px;color:#8aa0be;letter-spacing:2px}
    @media(max-width:760px){.tribun{grid-template-columns:1fr;grid-template-rows:55% 45%}.tribun-panel{grid-row:2;padding:18px;display:grid;grid-template-columns:1fr 1fr;gap:12px}.tribun-stage{grid-row:1}.tribun-brand,.tribun-passive,.tribun-foot{display:none}.tribun h1{font-size:36px}.tribun-intro{font-size:11px}.tribun-skills{grid-column:2;grid-row:1 / span 5}.tribun-skill{padding:8px}.tribun-caption{bottom:10px;left:20px}.tribun-caption p{font-size:16px}.tribun-status{display:none}.tribun-tools{top:14px;right:14px}}
  `;
  document.head.appendChild(style);
  const page = document.createElement('main'); page.className = 'tribun';
  page.innerHTML = `<aside class="tribun-panel"><p class="tribun-brand">LEAGUE / OF / VIBES</p>
    <div><span class="tribun-tag">NOUVEAU CHAMPION · MAGE</span><h1>Mélenchon.</h1><p class="tribun-intro">Le Tribun indigné.<br>La voie du milieu est désormais<br>une commission d’enquête.</p></div>
    <p class="tribun-passive"><strong>P / Indignation permanente</strong><br>Trois sorts bien sentis : un bouclier, de la vitesse et une cravate qui monte dans les tours.</p>
    <div class="tribun-skills">${(['Q', 'W', 'E', 'R'] as const).map((slot) => `<button class="tribun-skill" data-slot="${slot}" aria-pressed="false"><kbd>${slot}</kbd><span><strong>${def.abilities[slot].name}</strong><small>${({ Q: 'Projectile · ralentissement', W: 'Zone · silence', E: 'Téléportation · double holographique', R: 'Provocation · bouclier · décibels' })[slot]}</small></span></button>`).join('')}</div>
    <p class="tribun-foot">CARICATURE SATIRIQUE · MODÈLE ORIGINAL<br>Vitrine visuelle sans serveur. En partie, dégâts,<br>délais et contrôles sont validés par le serveur.</p></aside>
    <section class="tribun-stage" aria-label="Modèle 3D interactif de Mélenchon"><div class="tribun-status">LE TRIBUN / MODÈLE 11</div>
    <div class="tribun-tools"><button data-view="face">Portrait</button><button data-view="full">Silhouette</button><button data-action="run" aria-pressed="false">Marche</button></div>
    <div class="tribun-caption"><span>GLISSER POUR TOURNER · MOLETTE POUR ZOOMER</span><p aria-live="polite">« Je demande un rappel au règlement. »</p></div></section>`;
  document.body.appendChild(page);
  const stage = page.querySelector<HTMLElement>('.tribun-stage')!; stage.prepend(canvas);
  const caption = page.querySelector('.tribun-caption p')!;
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.shadowMap.enabled = true; renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  renderer.toneMapping = THREE.ACESFilmicToneMapping; renderer.toneMappingExposure = 1.25;
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(35, 1, 0.1, 80);
  camera.position.set(4.7, 3.3, 6.5);
  const controls = new OrbitControls(camera, canvas);
  controls.target.set(0, 1.3, 0); controls.enableDamping = true;
  controls.minDistance = 2; controls.maxDistance = 14; controls.maxPolarAngle = Math.PI * 0.49;
  scene.add(new THREE.HemisphereLight(0xcfe7ff, 0x28324b, 2.7));
  const key = new THREE.DirectionalLight(0xffecd7, 4); key.position.set(3, 6, 5); key.castShadow = true;
  key.shadow.mapSize.set(1024, 1024); scene.add(key);
  const rim = new THREE.DirectionalLight(0x70bcff, 3); rim.position.set(-4, 3, -3); scene.add(rim);
  const platformGeo = new THREE.CylinderGeometry(1.65, 1.75, 0.15, 64);
  const platformMat = new THREE.MeshStandardMaterial({ color: 0x1b2d45, roughness: 0.5, metalness: 0.25 });
  const platform = new THREE.Mesh(platformGeo, platformMat); platform.position.y = -0.08; platform.receiveShadow = true; scene.add(platform);
  const model = new MelenchonView(); model.object3d.rotation.y = -Math.PI / 2; scene.add(model.object3d);
  const snap: EntitySnapshot = { id: 1, kind: 'champion', team: 'blue', subtype: 'melenchon', x: 0, y: 0, facing: 0, hp: 1000, maxHp: 1000, anim: 'idle', buffs: [] };
  installMelenchonFx();
  const fx = installFx({ scene, camera, getEntityObject: (id) => id === 1 ? model.object3d : undefined }, { manualUpdate: true });
  let time = 0, last = performance.now(), ultUntil = 0, busyUntil = 0, running = false, busy = false, raf = 0;
  const skillButtons = [...page.querySelectorAll<HTMLButtonElement>('[data-slot]')];
  let sequence: { at: number; ev: GameEvent }[] = [];
  let projectileId = 1000;
  const cast = (slot: AbilitySlot): void => {
    if (time < busyUntil) return;
    busy = true;
    busyUntil = time + (slot === 'R' ? 4.6 : slot === 'E' ? 1.5 : 1);
    model.playAnim(`cast_${slot}`);
    caption.textContent = ({ Q: '« Vous allez répondre à la question ! »', W: 'Motion adoptée. Le silence aussi.', E: 'Même son double demande la parole.', R: '« LA RÉPUBLIQUE, C’EST MOI ! »' })[slot];
    for (const button of skillButtons) { button.setAttribute('aria-pressed', String(button.dataset.slot === slot)); button.disabled = true; }
    const origin = { x: model.object3d.position.x * 100, y: 0 }, target = { x: origin.x, y: -350 };
    const queue = (delay: number, ev: GameEvent): void => { sequence.push({ at: time + delay, ev }); };
    queue(0, { type: 'abilityCast', source: 1, championId: def.id, slot, abilityId: def.abilities[slot].id, targetPos: target });
    if (slot === 'Q') {
      const id = projectileId++;
      queue(0.2, { type: 'projectileSpawn', id, source: 1, fxKey: 'melenchon_q', from: origin, to: target, speed: 1750, width: 110 });
      queue(0.4, { type: 'projectileHit', id, pos: target, fxKey: 'melenchon_q' });
    } else if (slot === 'W') {
      queue(0.2, { type: 'areaEffect', source: 1, fxKey: 'melenchon_w_telegraph', pos: target, radius: 280, duration: 0.55 });
      queue(0.75, { type: 'areaEffect', source: 1, fxKey: 'melenchon_w', pos: target, radius: 280, duration: 0.25 });
    } else if (slot === 'E') {
      model.object3d.position.x = model.object3d.position.x > 0 ? 0 : 1.4;
      const to = { x: model.object3d.position.x * 100, y: 0 };
      queue(0, { type: 'dash', source: 1, from: origin, to, duration: 0, fxKey: 'melenchon_e' });
      queue(0, { type: 'areaEffect', source: 1, fxKey: 'melenchon_e_telegraph', pos: origin, radius: 240, duration: 1.2 });
      queue(1.2, { type: 'areaEffect', source: 1, fxKey: 'melenchon_e', pos: origin, radius: 240, duration: 0.25 });
    } else {
      ultUntil = time + 4.6;
      queue(0.6, { type: 'areaEffect', source: 1, fxKey: 'melenchon_r', pos: origin, radius: 500, duration: 4 });
    }
    sequence.sort((a, b) => a.at - b.at);
  };
  for (const button of skillButtons) button.onclick = () => cast(button.dataset.slot as AbilitySlot);
  for (const button of page.querySelectorAll<HTMLButtonElement>('[data-view]')) button.onclick = () => {
    const face = button.dataset.view === 'face';
    camera.position.set(face ? 0.9 : 4.7, face ? 2.6 : 3.3, face ? 3.2 : 6.5);
    controls.target.set(0, face ? 2.05 : 1.3, 0);
  };
  const run = page.querySelector<HTMLButtonElement>('[data-action=run]')!;
  run.onclick = () => { running = !running; run.setAttribute('aria-pressed', String(running)); model.playAnim(running ? 'run' : 'idle'); };
  const keyboard = (e: KeyboardEvent): void => {
    if (e.repeat || e.ctrlKey || e.altKey || e.metaKey) return;
    const slot = e.key.toUpperCase();
    if (slot === 'Q' || slot === 'W' || slot === 'E' || slot === 'R') { e.preventDefault(); cast(slot); }
  };
  addEventListener('keydown', keyboard);
  const observer = new ResizeObserver(() => { camera.aspect = stage.clientWidth / stage.clientHeight; camera.updateProjectionMatrix(); renderer.setSize(stage.clientWidth, stage.clientHeight); });
  observer.observe(stage);
  const frame = (now: number): void => {
    const dt = Math.min((now - last) / 1000, 0.05); last = now; time += dt;
    while (sequence.length && sequence[0]!.at <= time) fx.dispatcher.handle(sequence.shift()!.ev);
    snap.buffs = ultUntil > time && ultUntil - time < 4 ? [{ id: 'melenchon_r', remaining: ultUntil - time }] : [];
    if (busy && time >= busyUntil) { busy = false; model.playAnim(running ? 'run' : 'idle'); for (const button of skillButtons) { button.setAttribute('aria-pressed', 'false'); button.disabled = false; } }
    model.update(snap, dt); fx.update(dt); controls.update(); renderer.render(scene, camera);
    raf = requestAnimationFrame(frame);
  };
  raf = requestAnimationFrame(frame);
  addEventListener('pagehide', () => { cancelAnimationFrame(raf); observer.disconnect(); removeEventListener('keydown', keyboard); sequence = []; fx.dispose(); model.dispose(); controls.dispose(); platformGeo.dispose(); platformMat.dispose(); renderer.dispose(); }, { once: true });
}
