import { expect, test, vi } from 'vitest';
import html from '../index.html?raw';
import { wireSettingsForm } from './settings';

function loadSettingsForm() {
  document.documentElement.innerHTML = new DOMParser().parseFromString(html, 'text/html').documentElement.innerHTML;
  return document.querySelector<HTMLFormElement>('#settings form')!;
}

test('el botón por defecto del formulario (Enter) es Guardar, no Cerrar', () => {
  const form = loadSettingsForm();
  const byDefault = form.querySelector('button:not([type]), button[type="submit"], input[type="submit"]');
  expect(byDefault?.id).toBe('save-key');
  expect(document.querySelector<HTMLButtonElement>('#close-settings')!.type).toBe('button');
});

test('enviar el formulario (Enter en la clave) guarda y no cierra el diálogo', () => {
  const form = loadSettingsForm();
  const save = vi.fn();
  wireSettingsForm(form, save);
  let event: SubmitEvent | undefined;
  form.addEventListener('submit', (e) => (event = e as SubmitEvent));
  form.requestSubmit(document.querySelector<HTMLButtonElement>('#save-key')!);
  expect(save).toHaveBeenCalledTimes(1);
  expect(event?.defaultPrevented).toBe(true);
});
