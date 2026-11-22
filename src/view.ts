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
  | 'noApiKey'
  | 'archiveNotSupported';

/** Serializado con las claves PascalCase de la API de RA. */
export interface GameEntry {
  ID: number;
  Title: string;
  ConsoleID: number;
  NumAchievements: number;
  Hashes: string[];
}

/** Entrada de `API_GetGameHashes.php`. */
export interface GameHash {
  MD5: string;
  Name: string;
  Labels: string[];
}

/** Ficha de `API_GetGame.php` (solo lo que se muestra). */
export interface GameInfo {
  ImageIcon: string | null;
  ImageTitle: string | null;
  ImageIngame: string | null;
  ImageBoxArt: string | null;
  Publisher: string | null;
  Developer: string | null;
  Genre: string | null;
  Released: string | null;
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
  gameHashes: GameHash[];
  alternatives: [number, string][];
  message: MessageCode | null;
}

export type StatusGroup = 'ok' | 'missing' | 'other';

export const GAME_URL = 'https://retroachievements.org/game/';
export const MEDIA_URL = 'https://media.retroachievements.org';
/** Imagen genérica que RA usa cuando un juego no tiene la suya. */
const NO_IMAGE = '/Images/000002.png';
/** Con más hashes que estos, la lista empieza plegada. */
const OPEN_HASHES_MAX = 6;

export function statusLabel(s: Status, t: Translator): string {
  return t(`status.${s}`);
}

export function statusGroup(s: Status): StatusGroup {
  if (s === 'registered' || s === 'registeredNoAchievements') return 'ok';
  if (s === 'notRegistered') return 'missing';
  return 'other';
}

/** Sustituye `{nombre}` en un texto traducido. */
export function fmt(text: string, vars: Record<string, string | number>): string {
  return text.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
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

const SVG_NS = 'http://www.w3.org/2000/svg';
/** Iconos de trazo (24×24). */
const ICONS: Record<string, string> = {
  check: 'M5 12.5l4.5 4.5L19 7.5',
  cross: 'M6.5 6.5l11 11M17.5 6.5l-11 11',
  dash: 'M7 12h10',
  copy: 'M9 9h10v10H9zM5 15V5h10',
  trophy: 'M8 4h8v5a4 4 0 0 1-8 0zM8 6H5v1.5A3.5 3.5 0 0 0 8.5 11M16 6h3v1.5A3.5 3.5 0 0 1 15.5 11M12 13v4M8.5 20h7M10 17h4v3h-4z',
  external: 'M14 5h5v5M19 5l-8 8M17 14v5H5V7h5',
  cartridge: 'M6 3h9l3 3v14a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1zM9 8h6v6H9zM9 18v3M12 18v3M15 18v3',
};

export function icon(name: keyof typeof ICONS | string, cls = 'icon'): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('class', cls);
  svg.setAttribute('aria-hidden', 'true');
  const p = document.createElementNS(SVG_NS, 'path');
  p.setAttribute('d', ICONS[name] ?? '');
  svg.append(p);
  return svg;
}

const STATUS_ICON: Record<StatusGroup, string> = { ok: 'check', missing: 'cross', other: 'dash' };

function pill(r: CheckResult, t: Translator): HTMLElement {
  const p = el('span', `pill pill-${statusGroup(r.status)}`);
  p.append(icon(STATUS_ICON[statusGroup(r.status)]), el('span', undefined, statusLabel(r.status, t)));
  return p;
}

function copyButton(md5: string, t: Translator): HTMLButtonElement {
  const btn = el('button', 'copy');
  btn.type = 'button';
  btn.dataset.md5 = md5;
  btn.title = t('ui.copyHash');
  btn.append(icon('copy'), el('span', 'copy-label', t('ui.copyHash')));
  return btn;
}

function hashLine(md5: string, t: Translator): HTMLElement {
  const h = el('div', 'hash');
  h.append(el('code', undefined, md5), copyButton(md5, t));
  return h;
}

function consolePicker(r: CheckResult, t: Translator): HTMLElement {
  if (r.alternatives.length > 0 && r.consoleId !== null) {
    const sel = el('select', 'console-select');
    sel.title = t('ui.console');
    sel.setAttribute('aria-label', t('ui.console'));
    const all: [number, string][] = [[r.consoleId, r.consoleName ?? String(r.consoleId)], ...r.alternatives];
    for (const [id, label] of all) {
      const o = el('option', undefined, label);
      o.value = String(id);
      sel.append(o);
    }
    sel.value = String(r.consoleId);
    return sel;
  }
  return el('span', 'console', r.consoleName ?? '');
}

function fileLabel(r: CheckResult): string {
  return r.innerName ? `${r.fileName} › ${r.innerName}` : r.fileName;
}

function achievementsText(n: number, t: Translator): string {
  return fmt(t(n === 1 ? 'ui.achievementOne' : 'ui.achievements'), { n });
}

function hashList(r: CheckResult, t: Translator): HTMLElement {
  const det = el('details', 'hashes');
  det.open = r.gameHashes.length <= OPEN_HASHES_MAX;
  det.append(el('summary', undefined, fmt(t('ui.compatibleHashes'), { n: r.gameHashes.length })));
  const ul = el('ul');
  const mine = (r.md5 ?? '').toLowerCase();
  // El que coincide va primero.
  const sorted = [...r.gameHashes].sort(
    (a, b) => Number(b.MD5.toLowerCase() === mine) - Number(a.MD5.toLowerCase() === mine),
  );
  for (const h of sorted) {
    const isMatch = h.MD5.toLowerCase() === mine;
    const li = el('li', isMatch ? 'is-match' : undefined);
    const top = el('div', 'h-top');
    top.append(el('span', 'h-name', h.Name || h.MD5));
    for (const l of h.Labels ?? []) top.append(el('span', 'tag', l));
    if (isMatch) {
      const m = el('span', 'match-tag');
      m.append(icon('check'), el('span', undefined, t('ui.yourFile')));
      top.append(m);
    }
    li.append(top, el('code', undefined, h.MD5.toLowerCase()));
    ul.append(li);
  }
  det.append(ul);
  return det;
}

function gameCard(r: CheckResult, game: GameEntry, t: Translator): HTMLElement {
  const card = el('article', `card card-game status-${r.status}`);
  card.dataset.group = statusGroup(r.status);
  card.dataset.gameId = String(game.ID);

  const cover = el('div', 'cover');
  cover.append(icon('cartridge', 'icon cover-ph'));
  card.append(cover);

  const body = el('div', 'card-body');
  const head = el('div', 'card-head');
  head.append(el('h2', 'title', game.Title), pill(r, t));
  body.append(head);

  const meta = el('div', 'meta');
  meta.append(el('span', 'console', r.consoleName ?? ''));
  const ach = el('span', 'ach');
  ach.append(icon('trophy'), el('span', undefined, achievementsText(game.NumAchievements, t)));
  meta.append(ach, el('span', 'meta-extra'));
  body.append(meta);

  const facts = el('dl', 'facts');
  facts.append(el('dt', undefined, t('ui.yourFile')), el('dd', 'file', fileLabel(r)));
  if (r.md5) {
    const dd = el('dd');
    dd.append(hashLine(r.md5, t));
    facts.append(el('dt', undefined, t('ui.yourHash')), dd);
  }
  body.append(facts);

  if (r.gameHashes.length > 0) body.append(hashList(r, t));

  const actions = el('div', 'card-actions');
  const a = el('a', 'btn btn-ghost');
  a.href = '#';
  a.dataset.url = `${GAME_URL}${game.ID}`;
  a.append(el('span', undefined, t('ui.openGame')), icon('external'));
  actions.append(a);
  body.append(actions);

  card.append(body);
  return card;
}

function compactCard(r: CheckResult, t: Translator): HTMLElement {
  const card = el('article', `card card-compact status-${r.status}`);
  card.dataset.group = statusGroup(r.status);

  const body = el('div', 'card-body');
  const head = el('div', 'card-head');
  head.append(el('h2', 'file', fileLabel(r)), pill(r, t));
  body.append(head);

  if (r.consoleId !== null || r.md5) {
    const line = el('div', 'meta');
    if (r.consoleId !== null) line.append(consolePicker(r, t));
    if (r.md5) line.append(hashLine(r.md5, t));
    body.append(line);
  }

  let message: string | null = r.message ? t(`msg.${r.message}`) : null;
  if (!message && r.status === 'notRegistered' && r.consoleName) {
    message = fmt(t('msg.notFound'), { console: r.consoleName });
  }
  if (message) body.append(el('p', 'message', message));

  card.append(body);
  return card;
}

/** Construye la tarjeta de un resultado. Sin dependencias de Tauri. */
export function renderRow(r: CheckResult, t: Translator): HTMLElement {
  const card = r.game ? gameCard(r, r.game, t) : compactCard(r, t);
  card.dataset.sourcePath = r.sourcePath;
  if (r.innerName) card.dataset.innerName = r.innerName;
  return card;
}

function imagePath(info: GameInfo): string | null {
  for (const p of [info.ImageBoxArt, info.ImageTitle, info.ImageIcon]) {
    if (p && p !== NO_IMAGE) return p;
  }
  return null;
}

/** Año de una fecha de lanzamiento libre ("2002-11-18", "November 18, 2002"...). */
export function releaseYear(released: string | null): string | null {
  if (!released) return null;
  return released.match(/\b(19|20)\d{2}\b/)?.[0] ?? released;
}

/** Completa la ficha con portada y metadatos. `info` nulo deja el hueco de portada. */
export function fillGameInfo(card: HTMLElement, info: GameInfo | null, t: Translator): void {
  if (!info) return;
  const cover = card.querySelector<HTMLElement>('.cover');
  const path = imagePath(info);
  if (cover && path && !cover.querySelector('img')) {
    const img = el('img');
    img.alt = t('ui.coverAlt');
    img.loading = 'lazy';
    img.src = `${MEDIA_URL}${path}`;
    img.addEventListener('load', () => cover.classList.add('has-img'));
    img.addEventListener('error', () => img.remove());
    cover.append(img);
  }
  const extra = card.querySelector<HTMLElement>('.meta-extra');
  if (extra) {
    const who = [info.Publisher || info.Developer, releaseYear(info.Released)].filter((x): x is string => !!x);
    const nodes: HTMLElement[] = [];
    if (who.length > 0) nodes.push(el('span', undefined, who.join(', ')));
    if (info.Genre) nodes.push(el('span', 'tag', info.Genre));
    extra.replaceChildren(...nodes);
  }
}
