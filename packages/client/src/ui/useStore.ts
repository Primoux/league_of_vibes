import { useEffect, useState } from 'preact/hooks';
import { store, type ClientState } from '../state.ts';

export function useStore(): ClientState {
  const [s, set] = useState(store.state);
  useEffect(() => store.subscribe(set), []);
  return s;
}
