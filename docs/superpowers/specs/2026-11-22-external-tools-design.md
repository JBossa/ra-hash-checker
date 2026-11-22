# Herramientas externas (Dolphin) — diseño

## Por qué
Formatos como `.rvz`, `.gcz` y `.wia` (GameCube/Wii) no se pueden hashear sin reconstruir el disco, y hacerlo dentro de
la app exigiría incluir la clave común de Wii. Muchos usuarios ya tienen Dolphin, cuyo `DolphinTool` convierte esos
formatos a `.iso`. La app debe poder apoyarse en herramientas instaladas por el usuario, empezando por Dolphin, con
una estructura que permita añadir otras (la siguiente prevista: `chdman` de MAME para `.chd`).

## Objetivos
- Sección **Ajustes › Herramientas externas**: por herramienta, estado (conectada / no encontrada / no válida), ruta,
  extensiones que habilita, botones *Buscar automáticamente*, *Elegir…*, *Quitar* y enlace de descarga.
- Con Dolphin conectado, soltar un `.rvz`, `.gcz` o `.wia` da su resultado normal (registrado, etc.).
- Sin Dolphin, esos archivos muestran «Conecta Dolphin en Ajustes › Herramientas externas…» con un botón que abre
  esa sección.
- El panel «Formatos compatibles» muestra qué extensiones dependen de una herramienta.

## No objetivos
- Plugins de terceros o comandos definidos por el usuario (la elección fue «herramientas externas» integradas).
- `chdman` en esta ronda (la estructura lo admite; se añade después).
- Recuperar un `.wbfs` sin partición de actualizaciones (Dolphin tampoco puede: los datos no están).
- Cancelar una conversión en curso desde la UI.

## Anti-objetivos
- Ejecutar algo que no sea el ejecutable que el usuario eligió o que la app detectó y validó.
- Dejar `.iso` temporales en disco (se borran siempre, también tras errores o al reiniciar).
- Usar un shell: los argumentos van como vector, sin interpolación.

## Enfoques
1. **Elegido:** `DolphinTool convert -f iso` a un archivo temporal, hashear y borrar.
2. Leer la salida en streaming (tubería con nombre): el hash lee datos a ~250 MB del inicio y Dolphin no garantiza
   escribir en orden. Frágil; descartado.
3. Lector nativo de RVZ: requiere la clave común de Wii en un proyecto MIT. Descartado.

## Diseño

### Adaptadores (`src-tauri/src/tools/`)
Un `struct ToolSpec` por herramienta, en una tabla estática:
- `id: &str` (`"dolphin"`), `name: &str` (`"Dolphin (DolphinTool)"`), `exe_names: &[&str]` (`DolphinTool.exe`),
- `extensions: &[&str]` (`rvz`, `gcz`, `wia`), `consoles: &[u32]` (16, 19),
- `download_url` (`https://dolphin-emu.org/download/`),
- `validate(exe) -> bool`: ejecutarlo sin argumentos (timeout 5 s, sin ventana de consola) y exigir que la salida
  contenga `commands supported` y `convert`,
- `output_size(input) -> Option<u64>`: tamaño del `.iso` resultante leído de la cabecera:
  - RVZ/WIA: magia `RVZ\x01`/`WIA\x01`, `iso_file_size` u64 BE en 0x24;
  - GCZ: magia `0xB10BC001` (LE), `data_size` u64 LE en 0x10;
- `convert(exe, input, out_dir) -> Result<PathBuf, ToolError>`: `convert -i <in> -o <out>.iso -f iso -u <out_dir>/user`.

### Configuración
`tools.json` en `app_config_dir`: `{ "dolphin": "C:\\...\\DolphinTool.exe" }`. Al leerla, una ruta que ya no existe
cuenta como «no encontrada» (no se borra sola).

### Detección automática (Windows)
En orden: `PATH`; `%ProgramFiles%\Dolphin*\`, `%ProgramFiles(x86)%\Dolphin*\`, `%LOCALAPPDATA%\Programs\Dolphin*\`,
`%USERPROFILE%\scoop\apps\dolphin\current\`; y, para las versiones portables, carpetas `Dolphin*` hasta 2 niveles bajo
`Downloads`, `Desktop` y `Documents`. El primer candidato que pase `validate` se guarda.

### Comprobación de un archivo
En `check_one` (lib.rs), antes de `inputs::expand`, si la extensión pertenece a una herramienta:
- **Sin configurar o no válida** → fila `Unsupported` con `MessageCode::NeedsTool` y `tool: Some("dolphin")`.
- **Configurada** →
  1. Emitir `progress` `{ sourcePath, fileName, tool: "Dolphin" }`; la UI muestra una tarjeta «Convirtiendo con Dolphin…».
  2. Comprobar espacio libre en la carpeta temporal (`app_cache_dir/tmp`) ≥ `output_size` + 64 MB; si no,
     `MessageCode::NoDiskSpace` con los GB necesarios en el mensaje (campo `detail`).
  3. Un solo `convert` a la vez en toda la app (mutex en el estado de Tauri). El hijo se mata si el guard se suelta.
  4. Hashear el `.iso` **solo** para `consoles` del adaptador (checker: `check_only(input, &[16, 19], src)`); sin
     alternativas de consola en la fila.
  5. La fila conserva `fileName`/`sourcePath` del archivo original.
  6. Borrar el `.iso` y la carpeta `user` de Dolphin pase lo que pase; al arrancar, vaciar `tmp`.
- Fallo de conversión → `MessageCode::ToolFailed` (el texto de error de la herramienta va al log, no a la UI).

### Comandos Tauri
- `get_tools() -> Vec<ToolView { id, name, path, status, extensions, consoles, downloadUrl }>`
- `detect_tool(id) -> ToolView` (busca, valida, guarda si encuentra)
- `set_tool_path(id, path) -> ToolView` (valida; si no es válida, error `invalidTool` y no guarda)
- `clear_tool(id) -> ToolView`

### UI
- Ajustes: nueva sección con una tarjeta por herramienta (estado como `pill`, ruta en monoespaciada, botones).
  *Elegir…* abre el diálogo de archivos filtrado a `.exe`.
- Historial: tarjeta provisional por `sourcePath` al recibir `progress`, reemplazada por la del resultado.
- Mensaje `needsTool`: botón «Abrir herramientas externas» que abre Ajustes con la sección visible.
- Formatos compatibles: las extensiones de herramientas aparecen en GameCube y Wii con la marca «Dolphin».
- `opener`: permitir `https://dolphin-emu.org/*`.

## Riesgos
- Conversiones de Wii de doble capa: 8,5 GB temporales y unos minutos. Se avisa antes por espacio; no hay cancelación.
- Versiones antiguas de DolphinTool sin `-f iso`/`-u`: `validate` exige `convert`; si falla la conversión se ve
  `ToolFailed`.

## Para quien lo construya
Textos exactos (es/en), estilos de la tarjeta, nombres internos de funciones y la forma de los tests.
