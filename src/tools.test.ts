import { expect, test } from 'vitest';
import { makeTranslator } from './i18n';
import { renderPending, renderToolCard, toolExtensionsByConsole, type ToolView } from './tools';

const t = makeTranslator('es');
const dolphin: ToolView = {
  id: 'dolphin',
  name: 'Dolphin (DolphinTool)',
  shortName: 'Dolphin',
  path: null,
  status: 'notConfigured',
  extensions: ['rvz', 'gcz', 'wia'],
  consoles: ['GameCube', 'Wii'],
  downloadUrl: 'https://dolphin-emu.org/download/',
};

const actions = (el: HTMLElement) => Array.from(el.querySelectorAll<HTMLElement>('[data-action]')).map((b) => b.dataset.action);

test('sin configurar: buscar, elegir y descargar; sin quitar ni ruta', () => {
  const card = renderToolCard(dolphin, t);
  expect(card.dataset.toolId).toBe('dolphin');
  expect(card.textContent).toContain('Sin configurar');
  expect(card.textContent).toContain('.rvz .gcz .wia');
  expect(card.textContent).toContain('GameCube, Wii');
  expect(actions(card)).toEqual(['detect', 'choose', 'download']);
  expect(card.querySelector('.tool-path')).toBeNull();
});

test('conectada: muestra la ruta y permite cambiarla o quitarla', () => {
  const card = renderToolCard({ ...dolphin, status: 'connected', path: 'C:\\Dolphin\\DolphinTool.exe' }, t);
  expect(card.textContent).toContain('Conectada');
  expect(card.querySelector('.tool-path')?.textContent).toBe('C:\\Dolphin\\DolphinTool.exe');
  expect(actions(card)).toEqual(['choose', 'clear', 'download']);
});

test('no encontrada: avisa de que el archivo ya no está y ofrece buscar de nuevo', () => {
  const card = renderToolCard({ ...dolphin, status: 'missing', path: 'C:\\old\\DolphinTool.exe' }, t);
  expect(card.textContent).toContain('No encontrada');
  expect(card.querySelector('.tool-warn')).not.toBeNull();
  expect(actions(card)).toEqual(['detect', 'choose', 'clear', 'download']);
});

test('tarjeta provisional según la etapa', () => {
  const p = { sourcePath: 'C:/g.rvz', fileName: 'g.rvz', tool: 'Dolphin', stage: 'queued' as const };
  const queued = renderPending(p, t);
  expect(queued.dataset.sourcePath).toBe('C:/g.rvz');
  expect(queued.textContent).toContain('En cola para Dolphin');
  expect(renderPending({ ...p, stage: 'converting' }, t).textContent).toContain('Convirtiendo con Dolphin');
});

test('extensiones de herramientas por consola', () => {
  const m = toolExtensionsByConsole([dolphin]);
  expect(m.get('Wii')).toEqual([
    { ext: 'rvz', tool: 'Dolphin' },
    { ext: 'gcz', tool: 'Dolphin' },
    { ext: 'wia', tool: 'Dolphin' },
  ]);
  expect(m.has('PlayStation')).toBe(false);
});
