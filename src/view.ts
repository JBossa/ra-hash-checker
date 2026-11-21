import type { Translator } from './i18n';

export type Status =
  | 'registered'
  | 'registeredNoAchievements'
  | 'notRegistered'
  | 'unchecked'
  | 'unsupported'
  | 'error';

export type MessageCode =
  | 'discInZip'
  | 'noHash'
  | 'readError'
  | 'invalidApiKey'
  | 'offline'
  | 'noApiKey';

/** Serializado con las claves PascalCase de la API de RA. */
export interface GameEntry {
  ID: number;
  Title: string;
  ConsoleID: number;
  NumAchievements: number;
  Hashes: string[];
}

export interface CheckResult {
  fileName: string;
  sourcePath: string;
  innerName: string | null;
  consoleId: number | null;
  consoleName: string | null;
  md5: string | null;
  status: Status;
  game: GameEntry | null;
  matchedName: string | null;
  alternatives: [number, string][];
  message: MessageCode | null;
}

export const GAME_URL = 'https://retroachievements.org/game/';

export function statusLabel(s: Status, t: Translator): string {
  return t(`status.${s}`);
}

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  cls?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** Construye la fila de un resultado. Sin dependencias de Tauri. */
export function renderRow(r: CheckResult, t: Translator): HTMLElement {
  const row = el('div', `row status-${r.status}`);
  row.dataset.sourcePath = r.sourcePath;
  if (r.innerName) row.dataset.innerName = r.innerName;

  const name = r.innerName ? `${r.fileName} › ${r.innerName}` : r.fileName;
  row.append(el('div', 'file', name));

  const info = el('div', 'info');
  if (r.alternatives.length > 0 && r.consoleId !== null) {
    const sel = el('select', 'console-select');
    sel.title = t('ui.console');
    const all: [number, string][] = [[r.consoleId, r.consoleName ?? String(r.consoleId)], ...r.alternatives];
    for (const [id, label] of all) {
      const o = el('option', undefined, label);
      o.value = String(id);
      sel.append(o);
    }
    sel.value = String(r.consoleId);
    info.append(sel);
  } else if (r.consoleName) {
    info.append(el('span', 'console', r.consoleName));
  }
  row.append(info);

  if (r.md5) {
    const h = el('div', 'hash');
    h.append(el('code', undefined, r.md5));
    const btn = el('button', 'copy', t('ui.copyHash'));
    btn.type = 'button';
    btn.dataset.md5 = r.md5;
    h.append(btn);
    row.append(h);
  }

  const st = el('div', 'status');
  st.append(el('span', 'badge', statusLabel(r.status, t)));
  if (r.game) {
    st.append(' ', el('span', 'game', r.game.Title));
    const a = el('a', 'link', t('ui.openGame'));
    a.href = '#';
    a.dataset.url = `${GAME_URL}${r.game.ID}`;
    st.append(' ', a);
  }
  row.append(st);

  if (r.matchedName) row.append(el('div', 'matched', r.matchedName));
  if (r.message) row.append(el('div', 'message', t(`msg.${r.message}`)));

  return row;
}
