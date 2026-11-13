import { useState } from 'preact/hooks';

const assets = import.meta.glob<string>('../../assets/champions/*.webp', { eager: true, query: '?url', import: 'default' });

/** URLs Vite hashées : fonctionnent aussi quand l'application est hébergée sous un sous-chemin. */
export function championArtworkUrl(id: string): string | undefined {
  return assets[`../../assets/champions/${id}.webp`];
}

/** Le portrait SVG existant reste dessous et réapparaît si le chargement de l'image échoue. */
export function ChampionArtwork({ id, width, height }: { id: string; width: number; height: number }) {
  const source = championArtworkUrl(id);
  const [failed, setFailed] = useState<string>();
  if (!source || failed === source) return null;
  return <image href={source} width={width} height={height} preserveAspectRatio="xMidYMid slice" aria-hidden="true" onError={() => setFailed(source)} />;
}
