import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { makeTranslator } from './i18n';
import { renderRow, type CheckResult } from './view';
import { wireSettingsForm } from './settings';

const t = makeTranslator(navigator.language || 'en');
const KEYS_URL = 'https://retroachievements.org/controlpanel.php';

interface SettingsView {
  hasApiKey: boolean;
  lastRefresh: number | null;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const results = $('results');
const dialog = $<HTMLDialogElement>('settings');

function msg(e: unknown): string {
  const code = String(e);
  const m = t(`msg.${code}`);
  return m === `msg.${code}` ? code : m;
}

function applyStaticText() {
  $('drop-text').textContent = t('ui.drop');
  $('no-key-text').textContent = t('ui.noKeyNotice');
  $('get-key').textContent = t('ui.getKey');
  $('settings-btn').title = t('ui.settings');
  $('settings-title').textContent = t('ui.settings');
  $('key-label').textContent = t('ui.apiKey');
  $('save-key').textContent = t('ui.save');
  $('refresh').textContent = t('ui.refresh');
  $('close-settings').textContent = t('ui.close');
}

async function loadSettings(): Promise<SettingsView> {
  const s = await invoke<SettingsView>('get_settings');
  $('no-key').hidden = s.hasApiKey;
  $('key-hint').textContent = s.hasApiKey ? t('ui.keyStored') : '';
  $('last-refresh').textContent = `${t('ui.lastRefresh')}: ${
    s.lastRefresh ? new Date(s.lastRefresh * 1000).toLocaleString() : t('ui.never')
  }`;
  return s;
}

function addRow(r: CheckResult) {
  results.append(renderRow(r, t));
}

async function checkPaths(paths: string[]) {
  if (paths.length === 0) return;
  try {
    await invoke('check_paths', { paths });
  } catch (e) {
    for (const p of paths) {
      addRow({
        fileName: p.split(/[\\/]/).pop() || p,
        sourcePath: p,
        innerName: null,
        consoleId: null,
        consoleName: null,
        md5: null,
        status: 'error',
        game: null,
        matchedName: null,
        alternatives: [],
        message: null,
      });
    }
    console.error(e);
  }
}

// Enlaces y copiar hash (delegación de eventos).
results.addEventListener('click', async (ev) => {
  const target = ev.target as HTMLElement;
  const link = target.closest<HTMLElement>('[data-url]');
  if (link) {
    ev.preventDefault();
    await openUrl(link.dataset.url!);
    return;
  }
  const copy = target.closest<HTMLButtonElement>('button.copy');
  if (copy?.dataset.md5) {
    await writeText(copy.dataset.md5);
    const label = copy.textContent;
    copy.textContent = t('ui.copied');
    setTimeout(() => (copy.textContent = label), 1200);
  }
});

// Cambio de consola: se reemplaza la fila conservando la lista original.
results.addEventListener('change', async (ev) => {
  const sel = ev.target as HTMLSelectElement;
  if (!sel.matches('select.console-select')) return;
  const row = sel.closest<HTMLElement>('.row');
  if (!row) return;
  const consoles: [number, string][] = Array.from(sel.options).map((o) => [
    Number(o.value),
    o.textContent ?? o.value,
  ]);
  const consoleId = Number(sel.value);
  try {
    const r = await invoke<CheckResult>('check_as', {
      sourcePath: row.dataset.sourcePath,
      innerName: row.dataset.innerName ?? null,
      consoleId,
    });
    const current = consoles.find(([id]) => id === consoleId);
    if (current && r.consoleId === consoleId) {
      r.consoleName = r.consoleName ?? current[1];
      r.alternatives = consoles.filter(([id]) => id !== consoleId);
    }
    row.replaceWith(renderRow(r, t));
  } catch (e) {
    console.error(e);
  }
});

// Soltar y hacer clic en la zona.
const drop = $('drop');
drop.addEventListener('click', async (ev) => {
  if ((ev.target as HTMLElement).closest('a')) return;
  const picked = await open({ multiple: true });
  if (picked) await checkPaths(Array.isArray(picked) ? picked : [picked]);
});
$('get-key').addEventListener('click', async (ev) => {
  ev.preventDefault();
  await openUrl(KEYS_URL);
});

void getCurrentWebview().onDragDropEvent((ev) => {
  const p = ev.payload;
  if (p.type === 'enter' || p.type === 'over') drop.classList.add('over');
  else drop.classList.remove('over');
  if (p.type === 'drop') void checkPaths(p.paths);
});

void listen<CheckResult>('result', (ev) => addRow(ev.payload));

// Ajustes.
$('settings-btn').addEventListener('click', async () => {
  $<HTMLInputElement>('api-key').value = '';
  $('settings-msg').textContent = '';
  await loadSettings();
  dialog.showModal();
});
async function saveKey() {
  const input = $<HTMLInputElement>('api-key');
  try {
    await invoke('set_api_key', { key: input.value });
    input.value = '';
    $('settings-msg').textContent = t('ui.keySaved');
    await loadSettings();
  } catch (e) {
    $('settings-msg').textContent = msg(e);
  }
}
wireSettingsForm(dialog.querySelector('form')!, saveKey);
$('close-settings').addEventListener('click', () => dialog.close());
$('refresh').addEventListener('click', async () => {
  const btn = $<HTMLButtonElement>('refresh');
  btn.disabled = true;
  $('settings-msg').textContent = t('ui.refreshing');
  try {
    await invoke<number>('refresh_lists');
    $('settings-msg').textContent = t('ui.refreshed');
    await loadSettings();
  } catch (e) {
    $('settings-msg').textContent = msg(e);
  } finally {
    btn.disabled = false;
  }
});

document.documentElement.lang = t('ui.close') === 'Cerrar' ? 'es' : 'en';
applyStaticText();
void loadSettings();
