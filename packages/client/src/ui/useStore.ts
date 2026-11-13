import { useEffect, useRef, useState } from 'preact/hooks';
import { store, type ClientState } from '../state.ts';

/** Abonnement à tout l'état (à éviter dans le HUD : re-render à chaque snapshot). */
export function useStore(): ClientState {
  const [s, set] = useState(store.state);
  useEffect(() => store.subscribe(set), []);
  return s;
}

/** Égalité superficielle (objets/tableaux d'un niveau). */
export function shallowEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  if (typeof a !== 'object' || typeof b !== 'object' || !a || !b) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const ka = Object.keys(a);
  const kb = Object.keys(b);
  if (ka.length !== kb.length) return false;
  for (const k of ka) if (!Object.is((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k])) return false;
  return true;
}

/**
 * Sélecteur fin : le composant ne se re-rend que si la valeur sélectionnée change (égalité superficielle).
 * Indispensable pour ne pas tout re-rendre à 20 Hz.
 */
export function useSelect<T>(select: (s: ClientState) => T, eq: (a: T, b: T) => boolean = shallowEqual): T {
  const selRef = useRef(select);
  selRef.current = select;
  const [value, setValue] = useState(() => select(store.state));
  const valueRef = useRef(value);
  useEffect(() => {
    const check = (s: ClientState) => {
      const next = selRef.current(s);
      if (!eq(valueRef.current, next)) {
        valueRef.current = next;
        setValue(() => next);
      }
    };
    check(store.state);
    return store.subscribe(check);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return value;
}

/** Mini-store générique pour l'état propre à l'UI (panneaux ouverts, feed, réglages...). */
export class MiniStore<T extends object> {
  private listeners = new Set<(s: T) => void>();
  constructor(public state: T) {}
  set(patch: Partial<T>): void {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((l) => l(this.state));
  }
  update(fn: (s: T) => Partial<T>): void {
    this.set(fn(this.state));
  }
  subscribe(l: (s: T) => void): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }
}

export function useMini<T extends object, R>(ms: MiniStore<T>, select: (s: T) => R, eq: (a: R, b: R) => boolean = shallowEqual): R {
  const selRef = useRef(select);
  selRef.current = select;
  const [value, setValue] = useState(() => select(ms.state));
  const valueRef = useRef(value);
  useEffect(() => {
    const check = (s: T) => {
      const next = selRef.current(s);
      if (!eq(valueRef.current, next)) {
        valueRef.current = next;
        setValue(() => next);
      }
    };
    check(ms.state);
    return ms.subscribe(check);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ms]);
  return value;
}
