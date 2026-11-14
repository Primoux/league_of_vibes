/** Atelier autonome : ?modellab, comparaison entre le portrait livré et la véritable vue du jeu. */
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { CHAMPION_LIST, type EntitySnapshot } from '@lov/shared';
import { championArtworkUrl } from '../../../ui/icons/ChampionArtwork.tsx';
import { createChampionView, type AnimatedChampionView } from '../index.ts';

export function startModelLab(canvas: HTMLCanvasElement): void {
  document.title = 'Atelier des champions — League of Vibes';
  const style = document.createElement('style');
  style.textContent = `
  #ui{display:none}body{margin:0;overflow:hidden;background:#101925}
  .model-lab{position:fixed;inset:0;display:grid;grid-template-columns:340px 1fr;font:13px Arial,sans-serif;color:#f5eddd}
  .model-panel{padding:25px;background:#142135;border-right:1px solid #ffffff18;overflow:auto}
  .model-brand{font-size:9px;letter-spacing:3px;color:#a9bbd1;margin-bottom:22px}.model-panel h1{font:29px Georgia;margin:8px 0}.model-title{color:#adbbcd;font-size:12px;margin-bottom:16px}
  .model-reference{display:block;width:100%;height:200px;object-fit:cover;object-position:50% 30%;border-radius:7px;border:1px solid #ffffff22}
  .model-label{margin:16px 0 9px;color:#9eb2cb;font-size:9px;letter-spacing:2px;text-transform:uppercase}
  .model-grid{display:grid;grid-template-columns:repeat(4,1fr);gap:7px}.model-grid button{padding:0;overflow:hidden;background:#0d1726;border:1px solid #ffffff1c;border-radius:5px;cursor:pointer;color:#dfd9cc}
  .model-grid button[aria-pressed=true]{border-color:#ead5a1;box-shadow:0 0 0 1px #ead5a1}.model-grid img{display:block;width:100%;aspect-ratio:1;object-fit:cover}.model-grid span{display:block;padding:5px 0;font-size:9px}
  .model-stage{position:relative;min-width:0;background:radial-gradient(ellipse at 50% 35%,#314355,#101925 75%)}
  .model-stage canvas{position:absolute!important;inset:0!important;width:100%!important;height:100%!important}.model-tools{position:absolute;top:24px;right:25px;display:flex;gap:6px}
  .model-lab button{font-family:inherit}.model-tools button,.model-anims button{padding:9px 13px;color:#e9e5d9;background:#18283bdf;border:1px solid #ffffff27;border-radius:5px;cursor:pointer;font-size:11px}
  .model-tools button:hover,.model-anims button:hover{border-color:#d5c59c}.model-anims{position:absolute;bottom:55px;left:20px;right:20px;display:flex;justify-content:center;gap:6px;flex-wrap:wrap}
  .model-hint{position:absolute;bottom:21px;left:10px;right:10px;text-align:center;font-size:10px;letter-spacing:1px;color:#a5b8cb;pointer-events:none}
  .model-note{font-size:10px;line-height:1.6;color:#8fa5bc;margin:18px 0 0}
  @media(max-width:760px){.model-lab{grid-template-columns:1fr;grid-template-rows:58% 42%}.model-stage{grid-row:1}.model-panel{grid-row:2;padding:14px;display:grid;grid-template-columns:110px 1fr;gap:10px}.model-brand,.model-note,.model-label{display:none}.model-panel h1{font-size:20px}.model-title{font-size:10px}.model-reference{height:105px;width:105px}.model-grid{grid-column:2;grid-row:1 / span 5;gap:5px}.model-grid span{font-size:8px}.model-tools{top:12px;right:12px}.model-anims{bottom:38px;gap:4px}.model-anims button{padding:7px 9px}.model-hint{bottom:12px;font-size:9px}}
  `;
  document.head.appendChild(style);
  const page = document.createElement('main'); page.className = 'model-lab';
  page.innerHTML = `<aside class="model-panel"><div class="model-brand">LEAGUE / OF / VIBES</div><h1></h1><div class="model-title"></div>
    <img class="model-reference" alt=""/><div class="model-label">Choisir un champion</div><div class="model-grid">${CHAMPION_LIST.map(c => `<button data-champ="${c.id}" aria-label="${c.name}" aria-pressed="false"><img src="${championArtworkUrl(c.id)}" alt=""/><span>${c.name}</span></button>`).join('')}</div>
    <p class="model-note">Le modèle affiché est celui utilisé en partie, avec ses animations. Le portrait sert de référence au costume, au visage et à la silhouette.</p></aside>
    <section class="model-stage" aria-label="Modèle 3D du champion"><div class="model-tools"><button data-camera="front">Portrait</button><button data-camera="game">Vue de jeu</button></div>
    <div class="model-anims"><button data-anim="idle">Repos</button><button data-anim="run">Course</button><button data-anim="attack">Attaque</button>${['Q','W','E','R'].map(s=>`<button data-anim="cast_${s}">${s}</button>`).join('')}<button data-anim="death">Mort / retour</button></div>
    <div class="model-hint">GLISSER POUR TOURNER · MOLETTE POUR ZOOMER · Q W E R : ANIMATIONS</div></section>`;
  document.body.appendChild(page);
  const stage = page.querySelector<HTMLElement>('.model-stage')!; stage.prepend(canvas);
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true }); renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.shadowMap.enabled = true; renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  renderer.toneMapping = THREE.ACESFilmicToneMapping; renderer.toneMappingExposure = 1.1;
  const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(34, 1, .1, 80);
  camera.position.set(5.3, 2.9, -3.6);
  const controls = new OrbitControls(camera, canvas); controls.target.set(0, 1.15, 0); controls.enableDamping = true;
  controls.minDistance = 2; controls.maxDistance = 12; controls.maxPolarAngle = Math.PI * .49;
  scene.add(new THREE.HemisphereLight(0xdce8ff, 0x343548, 2.1));
  const key = new THREE.DirectionalLight(0xffe3bd, 3.1); key.position.set(4, 6, -5); key.castShadow = true; key.shadow.mapSize.set(1024, 1024); scene.add(key);
  const rim = new THREE.DirectionalLight(0x81b6ff, 2.2); rim.position.set(-3, 3, 4); scene.add(rim);
  const floorGeo = new THREE.CylinderGeometry(1.05, 1.12, .12, 64), floorMat = new THREE.MeshStandardMaterial({ color: 0x26394a, roughness: .58, metalness: .15 });
  const floor = new THREE.Mesh(floorGeo, floorMat); floor.position.y = -.065; floor.receiveShadow = true; scene.add(floor);
  const holder = new THREE.Group(); scene.add(holder);
  let view: AnimatedChampionView | null = null, walking = false, factor = 1, motion = 0, rUntil = 0;
  const snap: EntitySnapshot = { id: 1, kind: 'champion', team: 'blue', subtype: 'grondak', hp: 100, maxHp: 100, x: 0, y: 0, facing: 0 };
  const buttons = [...page.querySelectorAll<HTMLButtonElement>('[data-champ]')];
  const choose = (id: string): void => {
    view?.dispose(); holder.clear(); holder.position.set(0, 0, 0); walking = false; motion = 0; rUntil = 0;
    const def = CHAMPION_LIST.find(c => c.id === id) ?? CHAMPION_LIST[0]!;
    snap.subtype = def.id; snap.dead = false; snap.buffs = [];
    view = createChampionView({ ...snap })!; factor = 2 / view.height;
    holder.scale.setScalar(factor); holder.add(view.object3d);
    page.querySelector('h1')!.textContent = def.name; page.querySelector('.model-title')!.textContent = def.title;
    const image = page.querySelector<HTMLImageElement>('.model-reference')!; image.src = championArtworkUrl(def.id)!; image.alt = `Portrait de référence : ${def.name}`;
    for (const button of buttons) button.setAttribute('aria-pressed', String(button.dataset.champ === def.id));
  };
  for (const button of buttons) button.onclick = () => choose(button.dataset.champ!);
  const animate = (name: string): void => {
    if (!view) return;
    if (name === 'death') { snap.dead = !snap.dead; walking = false; view.playAnim(snap.dead ? 'death' : 'idle'); return; }
    snap.dead = false; view.update(snap, 0); walking = name === 'run';
    if (name.startsWith('cast_')) { view.cast(name.slice(5)); if (name === 'cast_R' && snap.subtype === 'melenchon') rUntil = performance.now() + 4000; }
    else if (name === 'attack') view.strike(); else view.playAnim(name);
  };
  for (const button of page.querySelectorAll<HTMLButtonElement>('[data-anim]')) button.onclick = () => animate(button.dataset.anim!);
  for (const button of page.querySelectorAll<HTMLButtonElement>('[data-camera]')) button.onclick = () => {
    const front = button.dataset.camera === 'front'; camera.position.set(5.5, front ? 2.1 : 5.4, front ? -.5 : -5.5); controls.target.set(0, front ? 1.2 : .9, 0);
  };
  const keyboard = (e: KeyboardEvent): void => { if (e.repeat || e.ctrlKey || e.altKey || e.metaKey) return; const key = e.key.toUpperCase(); if (['Q','W','E','R'].includes(key)) { e.preventDefault(); animate(`cast_${key}`); } };
  addEventListener('keydown', keyboard);
  const resize = new ResizeObserver(() => { camera.aspect = stage.clientWidth / stage.clientHeight; camera.updateProjectionMatrix(); renderer.setSize(stage.clientWidth, stage.clientHeight); }); resize.observe(stage);
  choose(new URLSearchParams(location.search).get('champ') ?? 'grondak');
  let last = performance.now(), raf = 0;
  const frame = (now: number): void => {
    const dt = Math.min(.05, (now - last) / 1000); last = now;
    if (view) {
      if (walking && !snap.dead) motion += dt * 3.4;
      view.object3d.position.x = motion; holder.position.x = -motion * factor;
      snap.buffs = rUntil > now ? [{ id: 'melenchon_r', remaining: (rUntil - now) / 1000 }] : [];
      view.update(snap, dt);
    }
    controls.update(); renderer.render(scene, camera); raf = requestAnimationFrame(frame);
  }; raf = requestAnimationFrame(frame);
  addEventListener('pagehide', () => { cancelAnimationFrame(raf); resize.disconnect(); removeEventListener('keydown', keyboard); view?.dispose(); controls.dispose(); floorGeo.dispose(); floorMat.dispose(); renderer.dispose(); }, { once: true });
}
