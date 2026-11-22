import type { Translator } from './i18n';
import { fmt } from './view';

/** Consola y extensiones que la app sabe comprobar (comando `supported_formats`). */
export interface ConsoleFormats {
  consoleId: number;
  consoleName: string;
  extensions: string[];
}

/** Normaliza la búsqueda: minúsculas, sin espacios ni punto inicial. */
export function normalizeQuery(q: string): string {
  return q.trim().toLowerCase().replace(/^\./, '');
}

/** Consolas cuyo nombre contiene la búsqueda o que aceptan esa extensión, por nombre. */
export function filterFormats(list: ConsoleFormats[], query: string): ConsoleFormats[] {
  const q = normalizeQuery(query);
  const sorted = [...list].sort((a, b) => a.consoleName.localeCompare(b.consoleName));
  if (!q) return sorted;
  return sorted.filter((c) => c.consoleName.toLowerCase().includes(q) || c.extensions.includes(q));
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** Filas de la lista; las extensiones que coinciden con la búsqueda se resaltan. */
export function renderFormats(list: ConsoleFormats[], query: string, t: Translator): HTMLElement[] {
  const q = normalizeQuery(query);
  const rows = filterFormats(list, query).map((c) => {
    const li = el('li', 'fmt-row');
    li.append(el('span', 'fmt-console', c.consoleName));
    const exts = el('span', 'fmt-exts');
    for (const e of c.extensions) exts.append(el('span', e === q ? 'ext is-hit' : 'ext', `.${e}`));
    li.append(exts);
    return li;
  });
  if (rows.length === 0 && q) {
    rows.push(el('li', 'fmt-empty', fmt(t('ui.formatsNone'), { q })));
  }
  return rows;
}
