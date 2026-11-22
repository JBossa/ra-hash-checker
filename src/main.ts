import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { makeTranslator } from './i18n';
import { fillGameInfo, renderRow, statusGroup, type CheckResult, type GameInfo, type StatusGroup } from './view';
import { wireSettingsForm } from './settings';
import { renderFormats, type ConsoleFormats } from './formats';
import {
  renderPending,
  renderToolCard,
  toolExtensionsByConsole,
  type ToolProgress,
  type ToolView,
} from './tools';

const t = makeTranslator(navigator.language || 'en');
const KEYS_URL = 'https://retroachievements.org/controlpanel.php';
const REPO_URL = 'https://github.com/JBossa/ra-hash-checker';

interface SettingsView {
  hasApiKey: boolean;
  lastRefresh: number | null;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const app = $('app');
const history = $('history');
const results = $('results');
const dialog = $<HTMLDialogElement>('settings');

function msg(e: unknown): string {
  const code = String(e);
  const m = t(`msg.${code}`);
  return m === `msg.${code}` ? code : m;
}

function applyStaticText() {
  $('drop-title').textContent = t('ui.dropTitle');
  $('drop-sub').textContent = t('ui.dropSub');
  $('drop-compact').textContent = t('ui.dropCompact');
  $('overlay-text').textContent = t('ui.dropOverlay');
  $('no-key-text').textContent = t('ui.noKeyNotice');
  $('add-key').textContent = t('ui.addKey');
  $('clear').textContent = t('ui.clear');
  $('busy-text').textContent = t('ui.checking');
  $('filter-empty').textContent = t('ui.filterEmpty');
  for (const s of document.querySelectorAll<HTMLElement>('[data-label]')) s.textContent = t(s.dataset.label!);
  const settingsBtn = $('settings-btn');
  settingsBtn.title = t('ui.settings');
  settingsBtn.setAttribute('aria-label', t('ui.settings'));
  $('close-settings').title = t('ui.close');
  $('close-settings').setAttribute('aria-label', t('ui.close'));
  $('settings-title').textContent = t('ui.settings');
  $('key-section').textContent = t('ui.keySection');
  $('key-help').textContent = t('ui.keyHelp');
  $<HTMLInputElement>('api-key').placeholder = t('ui.keyPlaceholder');
  $<HTMLInputElement>('api-key').setAttribute('aria-label', t('ui.keySection'));
  $('toggle-key').textContent = t('ui.show');
  $('save-key').textContent = t('ui.save');
  $('delete-key').textContent = t('ui.deleteKey');
  $('get-key').textContent = t('ui.getKey');
  $('lists-section').textContent = t('ui.listsSection');
  $('lists-help').textContent = t('ui.listsHelp');
  $('refresh').textContent = t('ui.refresh');
  $('about-section').textContent = t('ui.aboutSection');
  $('about-text').textContent = t('ui.about');
  $('source-link').textContent = t('ui.sourceCode');
  $('tools-title').textContent = t('ui.toolsSection');
  $('tools-help').textContent = t('ui.toolsHelp');
  $('formats-btn-text').textContent = t('ui.formats');
  $('formats-link').textContent = t('ui.formatsLink');
  $('formats-title').textContent = t('ui.formats');
  $('close-formats').title = t('ui.close');
  $('close-formats').setAttribute('aria-label', t('ui.close'));
  const q = $<HTMLInputElement>('formats-q');
  q.placeholder = t('ui.formatsSearch');
  q.setAttribute('aria-label', t('ui.formatsSearch'));
  const notes = ['ui.noteZip', 'ui.noteCompressed', 'ui.noteChd', 'ui.noteBin', 'ui.noteM3u', 'ui.noteArchives'];
  $('formats-notes').replaceChildren(
    ...notes.map((k) => {
      const li = document.createElement('li');
      li.textContent = t(k);
      return li;
    }),
  );
}

async function loadSettings(): Promise<SettingsView> {
  const s = await invoke<SettingsView>('get_settings');
  $('no-key').hidden = s.hasApiKey;
  const status = $('key-status');
  status.textContent = t(s.hasApiKey ? 'ui.keyStatusSet' : 'ui.keyStatusNone');
  status.className = `pill ${s.hasApiKey ? 'pill-ok' : 'pill-other'}`;
  $<HTMLButtonElement>('delete-key').hidden = !s.hasApiKey;
  $('last-refresh').textContent = `${t('ui.lastRefresh')}: ${
    s.lastRefresh ? new Date(s.lastRefresh * 1000).toLocaleString() : t('ui.never')
  }`;
  return s;
}

// --- Historial -------------------------------------------------------------------------------

/** Estado de cada tarjeta, para los contadores de los filtros. */
const groups = new Map<HTMLElement, StatusGroup>();
let filter: StatusGroup | 'all' = 'all';

function updateHistory() {
  const counts = { all: groups.size, ok: 0, missing: 0, other: 0 };
  for (const g of groups.values()) counts[g]++;
  for (const c of document.querySelectorAll<HTMLElement>('[data-count]')) {
    c.textContent = String(counts[c.dataset.count as keyof typeof counts]);
  }
  const empty = groups.size === 0;
  app.classList.toggle('is-empty', empty && pending === 0);
  history.hidden = empty && pending === 0;
  $('filter-empty').hidden = empty || filter === 'all' || counts[filter] > 0;
}

function setFilter(f: StatusGroup | 'all') {
  filter = f;
  results.dataset.filter = f;
  for (const b of document.querySelectorAll<HTMLElement>('.filter')) {
    const on = b.dataset.filter === f;
    b.classList.toggle('is-active', on);
    b.setAttribute('aria-pressed', String(on));
  }
  updateHistory();
}

/** Peticiones de ficha por juego: varias tarjetas del mismo juego comparten una. */
const infoRequests = new Map<number, Promise<GameInfo | null>>();

function loadGameInfo(card: HTMLElement, r: CheckResult) {
  if (!r.game) return;
  const id = r.game.ID;
  let req = infoRequests.get(id);
  if (!req) {
    req = invoke<GameInfo | null>('game_info', { gameId: id }).catch(() => null);
    infoRequests.set(id, req);
    // Si falló, permitir reintentar en la próxima coincidencia.
    void req.then((info) => info ?? infoRequests.delete(id));
  }
  void req.then((info) => fillGameInfo(card, info, t));
}

function showCard(r: CheckResult, replace?: HTMLElement): HTMLElement {
  const card = renderRow(r, t);
  card.classList.add('is-new');
  if (replace) {
    groups.delete(replace);
    replace.replaceWith(card);
  } else {
    results.prepend(card);
  }
  groups.set(card, statusGroup(r.status));
  loadGameInfo(card, r);
  updateHistory();
  return card;
}

/** Tarjetas provisionales de archivos que esperan o se convierten con una herramienta. */
const pendingCards = new Map<string, HTMLElement>();

function showPending(p: ToolProgress) {
  const card = renderPending(p, t);
  const old = pendingCards.get(p.sourcePath);
  if (old) old.replaceWith(card);
  else results.prepend(card);
  pendingCards.set(p.sourcePath, card);
  updateHistory();
}

function showResult(r: CheckResult) {
  const placeholder = pendingCards.get(r.sourcePath);
  pendingCards.delete(r.sourcePath);
  showCard(r, placeholder);
}

function clearHistory() {
  results.replaceChildren();
  groups.clear();
  pendingCards.clear();
  setFilter('all');
}

// --- Comprobación ----------------------------------------------------------------------------

let pending = 0;

function setBusy(delta: number) {
  pending += delta;
  $('busy').hidden = pending === 0;
  updateHistory();
}

function errorResult(p: string): CheckResult {
  return {
    fileName: p.split(/[\\/]/).pop() || p,
    sourcePath: p,
    innerName: null,
    consoleId: null,
    consoleName: null,
    md5: null,
    status: 'error',
    game: null,
    matchedName: null,
    gameHashes: [],
    alternatives: [],
    message: 'readError',
    tool: null,
    detail: null,
  };
}

async function checkPaths(paths: string[]) {
  if (paths.length === 0) return;
  setBusy(1);
  try {
    await invoke('check_paths', { paths });
  } catch (e) {
    for (const p of paths) showCard(errorResult(p));
    console.error(e);
  } finally {
    setBusy(-1);
  }
}

async function pickFiles() {
  const picked = await open({ multiple: true });
  if (picked) await checkPaths(Array.isArray(picked) ? picked : [picked]);
}

// Enlaces y copiar hash (delegación de eventos).
results.addEventListener('click', async (ev) => {
  const target = ev.target as HTMLElement;
  if (target.closest('[data-action="open-tools"]')) {
    void openSettings('tools');
    return;
  }
  const link = target.closest<HTMLElement>('[data-url]');
  if (link) {
    ev.preventDefault();
    await openUrl(link.dataset.url!);
    return;
  }
  const copy = target.closest<HTMLButtonElement>('button.copy');
  if (copy?.dataset.md5) {
    await writeText(copy.dataset.md5);
    const label = copy.querySelector('.copy-label');
    if (!label) return;
    copy.classList.add('is-done');
    label.textContent = t('ui.copied');
    setTimeout(() => {
      copy.classList.remove('is-done');
      label.textContent = t('ui.copyHash');
    }, 1200);
  }
});

// Cambio de consola: se reemplaza la tarjeta conservando la lista original.
results.addEventListener('change', async (ev) => {
  const sel = ev.target as HTMLSelectElement;
  if (!sel.matches('select.console-select')) return;
  const card = sel.closest<HTMLElement>('.card');
  if (!card) return;
  const consoles: [number, string][] = Array.from(sel.options).map((o) => [
    Number(o.value),
    o.textContent ?? o.value,
  ]);
  const consoleId = Number(sel.value);
  sel.disabled = true;
  try {
    const r = await invoke<CheckResult>('check_as', {
      sourcePath: card.dataset.sourcePath,
      innerName: card.dataset.innerName ?? null,
      consoleId,
    });
    const current = consoles.find(([id]) => id === consoleId);
    if (current && r.consoleId === consoleId) {
      r.consoleName = r.consoleName ?? current[1];
      r.alternatives = consoles.filter(([id]) => id !== consoleId);
    }
    showCard(r, card).classList.remove('is-new');
  } catch (e) {
    sel.disabled = false;
    console.error(e);
  }
});

for (const b of document.querySelectorAll<HTMLElement>('.filter')) {
  b.addEventListener('click', () => setFilter(b.dataset.filter as StatusGroup | 'all'));
}
$('clear').addEventListener('click', clearHistory);

// Zona de soltar: clic, teclado y arrastre sobre toda la ventana.
const drop = $('drop');
drop.addEventListener('click', () => void pickFiles());
drop.addEventListener('keydown', (ev) => {
  if (ev.key === 'Enter' || ev.key === ' ') {
    ev.preventDefault();
    void pickFiles();
  }
});

const overlay = $('overlay');
void getCurrentWebview().onDragDropEvent((ev) => {
  const p = ev.payload;
  overlay.hidden = !(p.type === 'enter' || p.type === 'over');
  if (p.type === 'drop') void checkPaths(p.paths);
});

void listen<CheckResult>('result', (ev) => showResult(ev.payload));
void listen<ToolProgress>('progress', (ev) => showPending(ev.payload));

// --- Formatos compatibles ------------------------------------------------------------------

const formatsDialog = $<HTMLDialogElement>('formats');
const formatsQuery = $<HTMLInputElement>('formats-q');
let formats: ConsoleFormats[] | null = null;

function showFormats() {
  if (!formats) return;
  $('formats-list').replaceChildren(...renderFormats(formats, formatsQuery.value, t, toolExtensionsByConsole(tools)));
}

async function openFormats(ev?: Event) {
  ev?.stopPropagation(); // el enlace está dentro de la zona de soltar
  formats ??= await invoke<ConsoleFormats[]>('supported_formats').catch(() => []);
  await loadTools();
  formatsQuery.value = '';
  showFormats();
  formatsDialog.showModal();
  formatsQuery.focus();
}

$('formats-btn').addEventListener('click', () => void openFormats());
$('formats-link').addEventListener('click', (ev) => void openFormats(ev));
$('formats-link').addEventListener('keydown', (ev) => ev.stopPropagation());
formatsQuery.addEventListener('input', showFormats);
$('close-formats').addEventListener('click', () => formatsDialog.close());
formatsDialog.addEventListener('click', (ev) => {
  if (ev.target === formatsDialog) formatsDialog.close();
});

// --- Ajustes ---------------------------------------------------------------------------------

const keyInput = $<HTMLInputElement>('api-key');
const settingsMsg = $('settings-msg');

function say(text: string, kind: 'ok' | 'error' | '' = '') {
  settingsMsg.textContent = text;
  settingsMsg.dataset.kind = kind;
}

function setKeyVisible(visible: boolean) {
  keyInput.type = visible ? 'text' : 'password';
  $('toggle-key').textContent = t(visible ? 'ui.hide' : 'ui.show');
}

async function openSettings(section?: 'tools') {
  keyInput.value = '';
  setKeyVisible(false);
  say('');
  await Promise.all([loadSettings(), loadTools()]);
  dialog.showModal();
  if (section === 'tools') $('tools-section').scrollIntoView({ block: 'start' });
  else keyInput.focus();
}

// --- Herramientas externas -------------------------------------------------------------------

let tools: ToolView[] = [];

function renderTools() {
  $('tools-list').replaceChildren(...tools.map((tool) => renderToolCard(tool, t)));
}

async function loadTools() {
  tools = await invoke<ToolView[]>('get_tools').catch(() => []);
  renderTools();
}

function updateTool(view: ToolView) {
  tools = tools.map((tool) => (tool.id === view.id ? view : tool));
  renderTools();
}

/** Ejecuta una acción sobre una herramienta y muestra el resultado en Ajustes. */
async function toolAction(run: () => Promise<ToolView>, done: string) {
  try {
    updateTool(await run());
    say(t(done), 'ok');
  } catch (e) {
    renderTools();
    say(msg(e), 'error');
  }
}

$('tools-list').addEventListener('click', async (ev) => {
  const button = (ev.target as HTMLElement).closest<HTMLButtonElement>('button[data-action]');
  const id = button?.closest<HTMLElement>('[data-tool-id]')?.dataset.toolId;
  const tool = tools.find((x) => x.id === id);
  if (!button || !id || !tool) return;
  switch (button.dataset.action) {
    case 'detect':
      button.disabled = true;
      button.textContent = t('ui.toolDetecting');
      await toolAction(() => invoke<ToolView>('detect_tool', { id }), 'ui.toolSaved');
      break;
    case 'choose': {
      const windows = navigator.userAgent.includes('Windows');
      const picked = await open({
        multiple: false,
        directory: false,
        filters: windows ? [{ name: tool.name, extensions: ['exe'] }] : undefined,
      });
      if (typeof picked === 'string') {
        await toolAction(() => invoke<ToolView>('set_tool_path', { id, path: picked }), 'ui.toolSaved');
      }
      break;
    }
    case 'clear':
      await toolAction(() => invoke<ToolView>('clear_tool', { id }), 'ui.toolCleared');
      break;
    case 'download':
      await openUrl(tool.downloadUrl);
      break;
  }
});

async function storeKey(key: string, done: string) {
  try {
    await invoke('set_api_key', { key });
    keyInput.value = '';
    say(t(done), 'ok');
    await loadSettings();
  } catch (e) {
    say(msg(e), 'error');
  }
}

async function saveKey() {
  const key = keyInput.value.trim();
  if (!key) {
    say(t('ui.keyEmpty'), 'error');
    keyInput.focus();
    return;
  }
  await storeKey(key, 'ui.keySaved');
}

$('settings-btn').addEventListener('click', () => void openSettings());
$('add-key').addEventListener('click', () => void openSettings());
$('close-settings').addEventListener('click', () => dialog.close());
// Clic fuera del panel lo cierra.
dialog.addEventListener('click', (ev) => {
  if (ev.target === dialog) dialog.close();
});
$('toggle-key').addEventListener('click', () => setKeyVisible(keyInput.type === 'password'));
wireSettingsForm(dialog.querySelector('form')!, saveKey);
$('delete-key').addEventListener('click', () => void storeKey('', 'ui.keyDeleted'));
$('get-key').addEventListener('click', async (ev) => {
  ev.preventDefault();
  await openUrl(KEYS_URL);
});
$('source-link').addEventListener('click', async (ev) => {
  ev.preventDefault();
  await openUrl(REPO_URL);
});
$('refresh').addEventListener('click', async () => {
  const btn = $<HTMLButtonElement>('refresh');
  btn.disabled = true;
  say(t('ui.refreshing'));
  try {
    await invoke<number>('refresh_lists');
    say(t('ui.refreshed'), 'ok');
    await loadSettings();
  } catch (e) {
    say(msg(e), 'error');
  } finally {
    btn.disabled = false;
  }
});

document.documentElement.lang = t('ui.close') === 'Cerrar' ? 'es' : 'en';
applyStaticText();
setFilter('all');
void loadSettings();
void getVersion().then((v) => ($('app-version').textContent = `v${v}`));
