import { expect, test } from 'vitest';
import { makeTranslator } from './i18n';
import {
  fillGameInfo,
  fmt,
  releaseYear,
  renderRow,
  statusGroup,
  statusLabel,
  type CheckResult,
  type GameInfo,
} from './view';

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
  gameHashes: [],
  alternatives: [],
  message: null,
  tool: null,
  detail: null,
};

const game = { ID: 1234, Title: 'Metroid Fusion', ConsoleID: 5, NumAchievements: 34, Hashes: [] };
const emptyInfo: GameInfo = {
  ImageIcon: null,
  ImageTitle: null,
  ImageIngame: null,
  ImageBoxArt: null,
  Publisher: null,
  Developer: null,
  Genre: null,
  Released: null,
};

test('statusLabel es', () => {
  const t = makeTranslator('es-CO');
  expect(statusLabel('registered', t)).toBe('Registrado');
  expect(statusLabel('registeredNoAchievements', t)).toBe('Registrado, sin logros todavía');
  expect(statusLabel('notRegistered', t)).toBe('No registrado');
  expect(statusLabel('unchecked', t)).toBe('Sin comparar');
});

test('statusGroup agrupa los estados para los filtros', () => {
  expect(statusGroup('registered')).toBe('ok');
  expect(statusGroup('registeredNoAchievements')).toBe('ok');
  expect(statusGroup('notRegistered')).toBe('missing');
  expect(statusGroup('unsupported')).toBe('other');
  expect(statusGroup('unchecked')).toBe('other');
  expect(statusGroup('error')).toBe('other');
});

test('fmt sustituye variables y deja las desconocidas', () => {
  expect(fmt('{n} logros de {x}', { n: 3 })).toBe('3 logros de {x}');
});

test('ficha muestra título, logros y enlace al juego', () => {
  const t = makeTranslator('es');
  const el = renderRow({ ...base, game }, t);
  expect(el.querySelector('.title')?.textContent).toBe('Metroid Fusion');
  expect(el.textContent).toContain('34 logros');
  expect(el.dataset.group).toBe('ok');
  expect(el.querySelector('[data-url="https://retroachievements.org/game/1234"]')).not.toBeNull();
});

test('ficha lista todos los hashes y resalta solo el que coincide, sin importar mayúsculas', () => {
  const t = makeTranslator('es');
  const el = renderRow(
    {
      ...base,
      game,
      gameHashes: [
        { MD5: '00000000000000000000000000000000', Name: 'Metroid Fusion (Europe)', Labels: ['nointro'] },
        { MD5: base.md5!.toUpperCase(), Name: 'Metroid Fusion (USA)', Labels: [] },
      ],
    },
    t,
  );
  const items = el.querySelectorAll('.hashes li');
  expect(items).toHaveLength(2);
  const matches = el.querySelectorAll('.hashes li.is-match');
  expect(matches).toHaveLength(1);
  expect(matches[0].textContent).toContain('Metroid Fusion (USA)');
  // El que coincide va primero.
  expect(items[0].classList.contains('is-match')).toBe(true);
  expect(el.textContent).toContain('Hashes compatibles (2)');
  expect(el.textContent).toContain('nointro');
});

test('ficha sin hashes del juego no muestra la lista', () => {
  const el = renderRow({ ...base, game }, makeTranslator('es'));
  expect(el.querySelector('.hashes')).toBeNull();
});

test('fillGameInfo nulo o sin imágenes deja el hueco de portada', () => {
  const t = makeTranslator('es');
  const el = renderRow({ ...base, game }, t);
  fillGameInfo(el, null, t);
  expect(el.querySelector('.cover img')).toBeNull();
  fillGameInfo(el, { ...emptyInfo, ImageBoxArt: '/Images/000002.png' }, t);
  expect(el.querySelector('.cover img')).toBeNull();
  expect(el.querySelector('.cover-ph')).not.toBeNull();
});

test('fillGameInfo pone la portada y los metadatos', () => {
  const t = makeTranslator('es');
  const el = renderRow({ ...base, game }, t);
  fillGameInfo(
    el,
    { ...emptyInfo, ImageBoxArt: '/Images/1.png', Publisher: 'Nintendo', Released: '2002-11-18 00:00:00' },
    t,
  );
  expect(el.querySelector('.cover img')?.getAttribute('src')).toBe('https://media.retroachievements.org/Images/1.png');
  const extra = el.querySelector('.meta-extra')!.textContent;
  expect(extra).toContain('Nintendo');
  expect(extra).toContain('2002');
  expect(extra).not.toContain('null');
});

test('fillGameInfo sin portada usa la pantalla de título', () => {
  const t = makeTranslator('es');
  const el = renderRow({ ...base, game }, t);
  fillGameInfo(el, { ...emptyInfo, ImageTitle: '/Images/7.png' }, t);
  expect(el.querySelector('.cover img')?.getAttribute('src')).toBe('https://media.retroachievements.org/Images/7.png');
});

test('releaseYear', () => {
  expect(releaseYear('November 18, 2002')).toBe('2002');
  expect(releaseYear(null)).toBeNull();
  expect(releaseYear('TBA')).toBe('TBA');
});

test('no registrado explica que ningún juego de la consola tiene el hash', () => {
  const el = renderRow({ ...base, status: 'notRegistered' }, makeTranslator('es'));
  expect(el.dataset.group).toBe('missing');
  expect(el.textContent).toContain('Ningún juego de Game Boy Advance en RetroAchievements tiene este hash.');
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
  const el = renderRow({ ...base, status: 'notRegistered', alternatives: [[9, 'Sega CD']] }, t);
  const opts = Array.from(el.querySelectorAll('select option')).map((o) => o.textContent);
  expect(opts).toEqual(['Game Boy Advance', 'Sega CD']);
});

test('row con archiveNotSupported muestra el mensaje en es y en', () => {
  const row = { ...base, status: 'unsupported' as const, md5: null, consoleId: null, consoleName: null, message: 'archiveNotSupported' as const };
  expect(renderRow(row, makeTranslator('es')).textContent).toContain('Descomprime el archivo y suelta la ROM');
  expect(renderRow(row, makeTranslator('en')).textContent).toContain('Extract the archive and drop the ROM');
});

test('needsTool nombra la herramienta y ofrece abrir la sección', () => {
  const el = renderRow({ ...base, status: 'unsupported', md5: null, consoleId: null, consoleName: null, message: 'needsTool', tool: 'Dolphin' }, makeTranslator('es'));
  expect(el.textContent).toContain('conecta Dolphin en Ajustes');
  expect(el.querySelector('[data-action="open-tools"]')).not.toBeNull();
});

test('noDiskSpace muestra los GB necesarios', () => {
  const el = renderRow({ ...base, status: 'error', md5: null, consoleId: null, consoleName: null, message: 'noDiskSpace', tool: 'Dolphin', detail: '4.8' }, makeTranslator('es'));
  expect(el.textContent).toContain('4.8 GB');
  expect(el.querySelector('[data-action="open-tools"]')).toBeNull();
});
