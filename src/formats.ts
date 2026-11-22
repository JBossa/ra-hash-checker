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

/** Extensiones que habilita una herramienta externa, por nombre de consola. */
export type ToolExtensions = Map<string, { ext: string; tool: string }[]>;

/** Consolas cuyo nombre contiene la búsqueda o que aceptan esa extensión, por nombre. */
export function filterFormats(list: ConsoleFormats[], query: string, toolExts: ToolExtensions = new Map()): ConsoleFormats[] {
  const q = normalizeQuery(query);
  const sorted = [...list].sort((a, b) => a.consoleName.localeCompare(b.consoleName));
  if (!q) return sorted;
  return sorted.filter(
    (c) =>
      c.consoleName.toLowerCase().includes(q) ||
      c.extensions.includes(q) ||
      (toolExts.get(c.consoleName) ?? []).some((x) => x.ext === q),
  );
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** Filas de la lista; las extensiones que coinciden con la búsqueda se resaltan. */
export function renderFormats(
  list: ConsoleFormats[],
  query: string,
  t: Translator,
  toolExts: ToolExtensions = new Map(),
): HTMLElement[] {
  const q = normalizeQuery(query);
  const rows = filterFormats(list, query, toolExts).map((c) => {
    const li = el('li', 'fmt-row');
    li.append(el('span', 'fmt-console', c.consoleName));
    const exts = el('span', 'fmt-exts');
    for (const e of c.extensions) exts.append(el('span', e === q ? 'ext is-hit' : 'ext', `.${e}`));
    for (const x of toolExts.get(c.consoleName) ?? []) {
      const chip = el('span', x.ext === q ? 'ext ext-tool is-hit' : 'ext ext-tool', `.${x.ext}`);
      chip.title = fmt(t('ui.needsToolChip'), { tool: x.tool });
      chip.append(el('span', 'ext-tool-name', x.tool));
      exts.append(chip);
    }
    li.append(exts);
    return li;
  });
  if (rows.length === 0 && q) {
    rows.push(el('li', 'fmt-empty', fmt(t('ui.formatsNone'), { q })));
  }
  return rows;
}
