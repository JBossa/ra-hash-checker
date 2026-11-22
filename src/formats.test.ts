import { expect, test } from 'vitest';
import { makeTranslator } from './i18n';
import { filterFormats, normalizeQuery, renderFormats, type ConsoleFormats } from './formats';

const list: ConsoleFormats[] = [
  { consoleId: 19, consoleName: 'Wii', extensions: ['iso', 'wad'] },
  { consoleId: 16, consoleName: 'GameCube', extensions: ['iso'] },
  { consoleId: 5, consoleName: 'Game Boy Advance', extensions: ['gba'] },
];

test('normalizeQuery quita el punto y los espacios', () => {
  expect(normalizeQuery('  .ISO ')).toBe('iso');
});

test('sin búsqueda devuelve todo ordenado por nombre', () => {
  expect(filterFormats(list, '').map((c) => c.consoleName)).toEqual(['Game Boy Advance', 'GameCube', 'Wii']);
});

test('busca por extensión exacta (con o sin punto) y por nombre', () => {
  expect(filterFormats(list, '.iso').map((c) => c.consoleId)).toEqual([16, 19]);
  expect(filterFormats(list, 'game').map((c) => c.consoleId)).toEqual([5, 16]);
  // Una extensión parcial no coincide con otra extensión.
  expect(filterFormats(list, 'is')).toEqual([]);
});

test('resalta la extensión buscada', () => {
  const rows = renderFormats(list, 'wad', makeTranslator('es'));
  expect(rows).toHaveLength(1);
  expect(rows[0].querySelector('.ext.is-hit')?.textContent).toBe('.wad');
});

test('extensión sin consola muestra un aviso con la extensión', () => {
  const rows = renderFormats(list, '.rvz', makeTranslator('es'));
  expect(rows).toHaveLength(1);
  expect(rows[0].textContent).toContain('.rvz');
});

test('las extensiones de herramientas se buscan y se marcan con su herramienta', () => {
  const toolExts = new Map([['Wii', [{ ext: 'rvz', tool: 'Dolphin' }]]]);
  expect(filterFormats(list, 'rvz', toolExts).map((c) => c.consoleId)).toEqual([19]);
  const rows = renderFormats(list, '.rvz', makeTranslator('es'), toolExts);
  const chip = rows[0].querySelector('.ext-tool');
  expect(chip?.classList.contains('is-hit')).toBe(true);
  expect(chip?.textContent).toBe('.rvzDolphin');
  expect(chip?.getAttribute('title')).toBe('Necesita Dolphin');
});
