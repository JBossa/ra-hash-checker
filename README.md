# RA Hash Checker

[Español](#español) · [English](#english)

## Español

Una app de escritorio sencilla para una sola cosa: sueltas el archivo de un juego en la ventana, calcula su hash de RetroAchievements (RA) y te dice si ese hash está registrado en RA y para qué juego.

### Qué hace

- Arrastra y suelta uno o varios archivos (o haz clic para elegirlos). Para cada uno muestra la consola detectada, el hash, y el resultado: registrado (con enlace a la página del juego), registrado sin logros todavía, no registrado, o sin comparar si no hay clave ni caché.
- Los hashes los calcula la librería oficial [rcheevos](https://github.com/RetroAchievements/rcheevos) (licencia MIT), la misma que usa RA.
- Los `.zip` se admiten para cartuchos (cada ROM dentro produce su propia fila) y para sets de arcade, que RA identifica por el nombre del zip.
- Las listas de hashes de cada consola se descargan al necesitarlas, se guardan en caché y se renuevan solas pasados 7 días (también hay un botón manual en Ajustes). Tras la primera descarga funciona sin conexión.
- Interfaz en español e inglés según el sistema.

### Clave de API

La app usa únicamente **tu propia** clave de la API web de RA y la API web pública; no incluye ni comparte ninguna clave común. Para obtenerla: [retroachievements.org](https://retroachievements.org) → panel de control → Keys (<https://retroachievements.org/controlpanel.php>). Pégala en Ajustes. Se guarda en el llavero del sistema, no en texto plano. Sin clave igualmente se calcula el hash, pero el resultado dice "sin comparar". Los archivos de tus juegos nunca se suben: solo se hacen peticiones de lectura a RA.

### Consolas con pruebas automáticas

GBA, GB/GBC, NES, SNES, Genesis, N64 y PS1 (cue/bin) tienen pruebas automáticas con archivos sintéticos; el hash de PS1 se verificó además con una imagen de disco real. rcheevos soporta más consolas; esas no tienen pruebas explícitas.

Cuando varias consolas comparten el mismo método de hash (por ejemplo Sega CD y Saturn, o los `.bin` de Mega Drive, 32X, Atari 2600 y otras), la app busca el hash en la lista de cada una, en orden, y muestra la primera que coincide.

### No-objetivos (v1)

- Renombrar, organizar, parchear o convertir ROMs.
- Archivos CHD, archivos 7z y juegos de 3DS cifrados.
- Cuentas, logros o progreso del usuario.
- Revisar una biblioteca completa por carpetas.
- Idea futura: sugerencia "¿Quizás…?" cuando el hash no coincide.

### Compilar desde el código fuente

Requisitos: Rust estable (en Windows, toolchain MSVC) y Node 24.

```
git clone --recursive https://github.com/<owner>/ra-hash-checker
cd ra-hash-checker
# si clonaste sin --recursive:
git submodule update --init
npm ci
npm run tauri dev     # modo desarrollo
npm run tauri build   # genera el instalador
```

Windows primero; Mac y Linux deben poder compilarse, pero no se han probado.

### Licencia

MIT, ver [LICENSE](LICENSE). Incluye rcheevos (MIT) como submódulo git.

## English

A simple desktop app for one thing: drop a game file into the window, it computes its RetroAchievements (RA) hash and tells you whether that hash is registered on RA, and for which game.

### What it does

- Drag and drop one or more files (or click to choose them). For each one it shows the detected console, the hash, and the result: registered (with a link to the game page), registered with no achievements yet, not registered, or not compared if there is no key or cache.
- Hashes are computed by the official [rcheevos](https://github.com/RetroAchievements/rcheevos) library (MIT license), the same one RA uses.
- `.zip` files are supported for cartridges (each ROM inside gets its own row) and for arcade sets, which RA identifies by the zip's name.
- Each console's hash list is downloaded when needed, cached, and refreshed automatically after 7 days (there is also a manual button in Settings). After the first download it works offline.
- Interface in Spanish and English, following the system language.

### API key

The app only uses **your own** RA web API key and the public web API; it ships no shared key. To get one: [retroachievements.org](https://retroachievements.org) → control panel → Keys (<https://retroachievements.org/controlpanel.php>). Paste it in Settings. It is stored in the OS keyring, not in plain text. Without a key the hash is still computed, but the result reads "not compared". Your game files are never uploaded: only read requests are sent to RA.

### Consoles with automated tests

GBA, GB/GBC, NES, SNES, Genesis, N64 and PS1 (cue/bin) are covered by automated tests with synthetic files; the PS1 hash was also verified with a real disc image. rcheevos supports more consoles; those have no explicit tests.

When several consoles share a hash method (for example Sega CD and Saturn, or `.bin` files for Mega Drive, 32X, Atari 2600 and others), the app looks the hash up in each console's list, in order, and shows the first one that matches.

### Non-goals (v1)

- Renaming, organizing, patching or converting ROMs.
- CHD files, 7z archives and encrypted 3DS games.
- User accounts, achievements or progress.
- Scanning a whole library by folder.
- Future idea: a "Maybe...?" suggestion when the hash does not match.

### Building from source

Requirements: stable Rust (MSVC toolchain on Windows) and Node 24.

```
git clone --recursive https://github.com/<owner>/ra-hash-checker
cd ra-hash-checker
# if you cloned without --recursive:
git submodule update --init
npm ci
npm run tauri dev     # development mode
npm run tauri build   # produces the installer
```

Windows first; Mac and Linux should build but are untested.

### License

MIT, see [LICENSE](LICENSE). Bundles rcheevos (MIT) as a git submodule.
