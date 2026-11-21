import { expect, test } from 'vitest';
import { makeTranslator } from './i18n';
import { renderRow, statusLabel, type CheckResult } from './view';

const base: CheckResult = {
  fileName: 'a.gba',
  sourcePath: 'C:/a.gba',
  innerName: null,
  consoleId: 5,
  consoleName: 'Game Boy Advance',
  md5: 'abcdef0123456789abcdef0123456789',
  status: 'registered',
  game: null,
  matchedName: null,
  alternatives: [],
  message: null,
};

test('statusLabel es', () => {
  const t = makeTranslator('es-CO');
  expect(statusLabel('registered', t)).toBe('✅ Registrado');
  expect(statusLabel('registeredNoAchievements', t)).toBe('🟡 Registrado, sin logros todavía');
  expect(statusLabel('notRegistered', t)).toBe('❌ No registrado');
  expect(statusLabel('unchecked', t)).toBe('⚪ Sin comparar');
});

test('row muestra juego y enlace', () => {
  const t = makeTranslator('es');
  const el = renderRow({ ...base, game: { ID: 1234, Title: 'X', ConsoleID: 5, NumAchievements: 3, Hashes: [] } }, t);
  expect(el.textContent).toContain('X');
  expect(el.querySelector('[data-url="https://retroachievements.org/game/1234"]')).not.toBeNull();
});

test('row con discInZip muestra el mensaje', () => {
  const t = makeTranslator('es');
  const el = renderRow({ ...base, status: 'error', message: 'discInZip' }, t);
  expect(el.textContent).toContain('Descomprime el disco y suéltalo de nuevo');
});

test('row unsupported sin mensaje muestra etiqueta localizada', () => {
  const t = makeTranslator('es');
  const el = renderRow({ ...base, status: 'unsupported' }, t);
  expect(el.textContent).toContain('Formato no soportado');
});

test('row con alternatives muestra selector', () => {
  const t = makeTranslator('es');
  const el = renderRow({ ...base, alternatives: [[9, 'Sega CD']] }, t);
  const opts = Array.from(el.querySelectorAll('select option')).map((o) => o.textContent);
  expect(opts).toEqual(['Game Boy Advance', 'Sega CD']);
});

test('row con archiveNotSupported muestra el mensaje en es y en', () => {
  const row = { ...base, status: 'unsupported' as const, md5: null, consoleId: null, consoleName: null, message: 'archiveNotSupported' as const };
  expect(renderRow(row, makeTranslator('es')).textContent).toContain('Descomprime el archivo y suelta la ROM');
  expect(renderRow(row, makeTranslator('en')).textContent).toContain('Extract the archive and drop the ROM');
});
