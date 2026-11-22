import { expect, test } from 'vitest';
import { makeTranslator } from './i18n';

test('es usa textos en español', () => {
  const t = makeTranslator('es-CO');
  expect(t('status.notRegistered')).toBe('No registrado');
  expect(t('status.unsupported')).toBe('Formato no soportado');
});

test('fallback en', () => {
  expect(makeTranslator('fr')('status.notRegistered')).toBe('Not registered');
});

test('clave desconocida devuelve la clave', () => {
  expect(makeTranslator('en')('nope.nope')).toBe('nope.nope');
});
