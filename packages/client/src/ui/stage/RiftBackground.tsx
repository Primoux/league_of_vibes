/**
 * Fond animé des menus (canvas 2D) : nébuleuse lente, faille lumineuse en diagonale (la rivière de la carte),
 * braises qui montent et constellations scintillantes. Teinte optionnelle (couleur du champion en vedette).
 */
import { useEffect, useRef } from 'preact/hooks';

interface Ember {
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
  life: number;
  hue: number;
}

export function RiftBackground({ tint = '#3ee6c8', intensity = 1 }: { tint?: string; intensity?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const tintRef = useRef(tint);
  tintRef.current = tint;

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const g = canvas.getContext('2d');
    if (!g) return;
    let w = 0;
    let h = 0;
    let raf = 0;
    const dpr = Math.min(1.5, devicePixelRatio || 1);
    const embers: Ember[] = [];
    const stars: { x: number; y: number; r: number; p: number }[] = [];
    const resize = () => {
      w = canvas.clientWidth;
      h = canvas.clientHeight;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      g.setTransform(dpr, 0, 0, dpr, 0, 0);
      stars.length = 0;
      for (let i = 0; i < Math.round((w * h) / 9000); i++) stars.push({ x: Math.random() * w, y: Math.random() * h, r: Math.random() * 1.3 + 0.2, p: Math.random() * Math.PI * 2 });
    };
    const spawn = (anywhere = false): Ember => ({
      x: Math.random() * w,
      y: anywhere ? Math.random() * h : h + 10,
      vx: (Math.random() - 0.5) * 12,
      vy: -(14 + Math.random() * 38),
      r: 0.6 + Math.random() * 2.2,
      life: Math.random(),
      hue: Math.random(),
    });
    const sprite = (col: string): HTMLCanvasElement => {
      const c = document.createElement('canvas');
      c.width = c.height = 32;
      const x = c.getContext('2d')!;
      const gr = x.createRadialGradient(16, 16, 0, 16, 16, 16);
      gr.addColorStop(0, 'rgba(255,255,255,1)');
      gr.addColorStop(0.18, col);
      gr.addColorStop(1, 'rgba(0,0,0,0)');
      x.fillStyle = gr;
      x.fillRect(0, 0, 32, 32);
      return c;
    };
    const sprites = { gold: sprite('rgba(246,210,120,0.7)'), tint: sprite(hexA(tintRef.current, 0.7)), violet: sprite('rgba(176,114,255,0.7)') };
    resize();
    const count = Math.round(70 * intensity);
    for (let i = 0; i < count; i++) embers.push(spawn(true));
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);

    let last = performance.now();
    let drawnOnce = false;
    const draw = (now: number) => {
      raf = requestAnimationFrame(draw);
      if (document.hidden || (drawnOnce && document.documentElement.classList.contains('lov-still'))) return;
      drawnOnce = true;
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      const t = now / 1000;
      g.clearRect(0, 0, w, h);

      // faille lumineuse : diagonale bas-gauche -> haut-droite qui ondule
      g.save();
      g.globalCompositeOperation = 'lighter';
      const grad = g.createLinearGradient(0, h, w, 0);
      grad.addColorStop(0, 'rgba(0,0,0,0)');
      grad.addColorStop(0.35, hexA(tintRef.current, 0.07 * intensity));
      grad.addColorStop(0.5, hexA(tintRef.current, 0.16 * intensity));
      grad.addColorStop(0.65, 'rgba(176,114,255,0.08)');
      grad.addColorStop(1, 'rgba(0,0,0,0)');
      g.strokeStyle = grad;
      g.lineCap = 'round';
      for (let k = 0; k < 3; k++) {
        g.lineWidth = (k === 0 ? 120 : k === 1 ? 40 : 6) * (1 + Math.sin(t * 0.7 + k) * 0.08);
        g.globalAlpha = k === 2 ? 0.9 : 0.6;
        g.beginPath();
        for (let i = 0; i <= 24; i++) {
          const f = i / 24;
          const x = f * w;
          const y = h - f * h + Math.sin(f * 7 + t * 0.6 + k) * 18 + Math.sin(f * 2.3 - t * 0.3) * 30;
          if (i) g.lineTo(x, y);
          else g.moveTo(x, y);
        }
        g.stroke();
      }
      g.restore();

      // étoiles scintillantes
      g.fillStyle = '#e8e4ff';
      for (const s of stars) {
        g.globalAlpha = 0.15 + 0.35 * (0.5 + 0.5 * Math.sin(t * 1.3 + s.p));
        g.fillRect(s.x, s.y, s.r * 0.8, s.r * 0.8);
      }

      // braises
      g.globalCompositeOperation = 'lighter';
      for (let i = 0; i < embers.length; i++) {
        const e = embers[i]!;
        e.x += (e.vx + Math.sin(t + e.hue * 10) * 6) * dt;
        e.y += e.vy * dt;
        e.life += dt * 0.12;
        if (e.y < -10 || e.life > 1.6) {
          embers[i] = spawn();
          continue;
        }
        const a = Math.max(0, Math.sin(Math.min(1, e.life) * Math.PI)) * 0.9;
        const sprite = e.hue < 0.55 ? sprites.gold : e.hue < 0.8 ? sprites.tint : sprites.violet;
        const size = e.r * 7;
        g.globalAlpha = a;
        g.drawImage(sprite, e.x - size / 2, e.y - size / 2, size, size);
      }
      g.globalAlpha = 1;
      g.globalCompositeOperation = 'source-over';
    };
    raf = requestAnimationFrame(draw);
    // première image même en mode figé (captures)
    draw(performance.now());
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
  }, [intensity]);

  return <canvas ref={ref} class="lov-rift-bg" aria-hidden="true" />;
}

function hexA(hex: string, a: number): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex);
  if (!m) return `rgba(62,230,200,${a})`;
  const n = parseInt(m[1]!, 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
}
