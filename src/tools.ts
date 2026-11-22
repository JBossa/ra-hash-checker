import type { Translator } from './i18n';
import { fmt, icon } from './view';

/** Estado de una herramienta externa (comando `get_tools`). */
export interface ToolView {
  id: string;
  name: string;
  shortName: string;
  path: string | null;
  status: 'connected' | 'missing' | 'notConfigured';
  extensions: string[];
  consoles: string[];
  downloadUrl: string;
}

/** Evento `progress`: un archivo espera o se está convirtiendo con una herramienta. */
export interface ToolProgress {
  sourcePath: string;
  fileName: string;
  tool: string;
  stage: 'queued' | 'converting';
}

export type ToolAction = 'detect' | 'choose' | 'clear' | 'download';

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

const STATUS_PILL: Record<ToolView['status'], string> = {
  connected: 'pill pill-ok',
  missing: 'pill pill-missing',
  notConfigured: 'pill pill-other',
};
const STATUS_TEXT: Record<ToolView['status'], string> = {
  connected: 'ui.toolConnected',
  missing: 'ui.toolMissing',
  notConfigured: 'ui.toolNotConfigured',
};

function actionButton(action: ToolAction, label: string, cls = 'btn btn-ghost btn-sm'): HTMLButtonElement {
  const b = el('button', cls, label);
  b.type = 'button';
  b.dataset.action = action;
  return b;
}

/** Tarjeta de una herramienta en Ajustes; los botones llevan `data-action`. */
export function renderToolCard(tool: ToolView, t: Translator): HTMLElement {
  const card = el('div', `tool-card tool-${tool.status}`);
  card.dataset.toolId = tool.id;

  const head = el('div', 'section-head');
  head.append(el('h4', undefined, tool.name), el('span', STATUS_PILL[tool.status], t(STATUS_TEXT[tool.status])));
  card.append(head);

  const exts = tool.extensions.map((e) => `.${e}`).join(' ');
  card.append(el('p', 'hint', fmt(t('ui.toolEnables'), { exts, consoles: tool.consoles.join(', ') })));

  if (tool.path) {
    card.append(el('code', 'tool-path', tool.path));
    if (tool.status === 'missing') card.append(el('p', 'hint tool-warn', t('ui.toolMissingHint')));
  }

  const actions = el('div', 'actions');
  if (tool.status !== 'connected') actions.append(actionButton('detect', t('ui.toolDetect'), 'btn btn-gold btn-sm'));
  actions.append(actionButton('choose', t('ui.toolChoose')));
  if (tool.path) actions.append(actionButton('clear', t('ui.toolClear')));
  const dl = actionButton('download', t('ui.toolDownload'), 'link link-btn-inline');
  dl.append(icon('external'));
  actions.append(dl);
  card.append(actions);
  return card;
}

/** Tarjeta provisional del historial mientras un archivo espera o se convierte. */
export function renderPending(p: ToolProgress, t: Translator): HTMLElement {
  const card = el('article', 'card card-compact card-pending');
  card.dataset.group = 'pending';
  card.dataset.sourcePath = p.sourcePath;
  const body = el('div', 'card-body');
  const head = el('div', 'card-head');
  head.append(el('h2', 'file', p.fileName));
  body.append(head);
  const line = el('p', 'message pending-line');
  line.append(el('span', 'spinner'));
  const key = p.stage === 'queued' ? 'ui.toolQueued' : 'ui.toolConverting';
  line.append(el('span', undefined, fmt(t(key), { tool: p.tool })));
  body.append(line);
  card.append(body);
  return card;
}

/** Extensiones que habilitan las herramientas, por nombre de consola (para «Formatos compatibles»). */
export function toolExtensionsByConsole(tools: ToolView[]): Map<string, { ext: string; tool: string }[]> {
  const out = new Map<string, { ext: string; tool: string }[]>();
  for (const tool of tools) {
    for (const console of tool.consoles) {
      const list = out.get(console) ?? [];
      for (const ext of tool.extensions) list.push({ ext, tool: tool.shortName });
      out.set(console, list);
    }
  }
  return out;
}
