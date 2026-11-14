/**
 * Chat en jeu : Entrée = équipe, Maj + Entrée = tous. Les messages s'estompent après 10 s
 * (l'historique complet réapparaît quand la saisie est ouverte).
 */
import { CHAMPIONS } from '@lov/shared';
import { useEffect, useRef, useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { chat } from '../game/world.ts';
import { ChampionPortrait } from '../icons/Icons.tsx';
import { useMini } from '../useStore.ts';

const VISIBLE_MS = 10_000;

export function Chat() {
  const st = useMini(chat, (s) => s, (a, b) => a === b);
  const [now, setNow] = useState(Date.now());
  const [text, setText] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Entrée ouvre la saisie (sauf si on tape déjà ailleurs, ex. recherche de la boutique)
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key !== 'Enter' || chat.state.open) return;
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) return;
      e.preventDefault();
      chat.set({ open: true, all: e.shiftKey });
    };
    addEventListener('keydown', down);
    return () => removeEventListener('keydown', down);
  }, []);

  useEffect(() => {
    if (st.open) inputRef.current?.focus();
  }, [st.open]);

  // rafraîchit l'estompage tant qu'il reste des messages visibles
  useEffect(() => {
    const last = st.lines[st.lines.length - 1];
    if (!last || Date.now() - last.at > VISIBLE_MS) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [st.lines]);

  useEffect(() => {
    listRef.current?.scrollTo({ top: listRef.current.scrollHeight });
  }, [st.lines, st.open]);

  const close = () => {
    setText('');
    chat.set({ open: false });
    inputRef.current?.blur();
  };
  const send = () => {
    const msg = text.trim().slice(0, 200);
    if (msg) net.send({ t: 'chat', text: msg, teamOnly: !chat.state.all });
    close();
  };

  const players = store.state.game?.players ?? [];
  const myTeam = store.state.game?.team;
  const lines = st.open ? st.lines.slice(-30) : st.lines.filter((l) => now - l.at < VISIBLE_MS).slice(-7);

  return (
    <div class={`lov-chat ${st.open ? 'open' : ''}`}>
      <div class="lov-chat-lines" ref={listRef}>
        {lines.map((l) => {
          const p = players.find((x) => x.id === l.playerId);
          const ally = p?.team === myTeam;
          return (
            <div key={l.key} class={`lov-chat-line ${ally ? 'ally' : 'enemy'}`}>
              {p && <ChampionPortrait id={p.championId} size={18} variant="round" />}
              <span class="tag">{l.teamOnly ? '[Équipe]' : '[Tous]'}</span>
              <b>
                {p?.name ?? 'Invocateur'}
                {p && <small> ({CHAMPIONS[p.championId]?.name ?? p.championId})</small>}
              </b>
              <span class="txt">{l.text}</span>
            </div>
          );
        })}
      </div>
      {st.open && (
        <div class="lov-chat-input">
          <button class={`scope ${st.all ? 'all' : ''}`} onClick={() => chat.set({ all: !st.all })} title="Maj + Entrée : tout le monde">
            {st.all ? 'Tous' : 'Équipe'}
          </button>
          <input
            ref={inputRef}
            value={text}
            maxLength={200}
            placeholder={st.all ? 'Message à tout le monde…' : 'Message à ton équipe…'}
            onInput={(e) => setText(e.currentTarget.value)}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === 'Enter') send();
              else if (e.key === 'Escape') close();
              else if (e.key === 'Tab') {
                e.preventDefault();
                chat.set({ all: !chat.state.all });
              }
            }}
            onBlur={() => !text && close()}
          />
        </div>
      )}
    </div>
  );
}
