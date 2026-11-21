/**
 * Enter en el campo de la clave envía el formulario por su botón por defecto (Guardar).
 * Se intercepta el envío para guardar sin cerrar el diálogo. Sin dependencias de Tauri.
 */
export function wireSettingsForm(form: HTMLFormElement, save: () => unknown): void {
  form.addEventListener('submit', (ev) => {
    ev.preventDefault();
    void save();
  });
}
