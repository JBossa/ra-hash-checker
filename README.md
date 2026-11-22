# RA Hash Checker: RetroAchievements ROM hash checker

[![CI](https://github.com/JBossa/ra-hash-checker/actions/workflows/ci.yml/badge.svg)](https://github.com/JBossa/ra-hash-checker/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Platform: Windows](https://img.shields.io/badge/platform-Windows-0078D6)
![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB)
![Rust](https://img.shields.io/badge/Rust-stable-orange)

**Drag and drop a ROM or disc image to see whether your copy matches a hash registered on [RetroAchievements](https://retroachievements.org).** RA Hash Checker is a small, open-source desktop app. It computes the official RetroAchievements hash of your game file for GBA, PS1, SNES, NES, N64, Genesis/Mega Drive, Game Boy and many more systems, and checks it against the list of hashes RetroAchievements supports for that game.

[English](#english) · [Español](#español)

---

## English

### Why

RetroAchievements only unlocks achievements when your ROM or disc image is one of the versions it supports. Finding out which version you have usually means a command-line tool or reading hash tables by hand. RA Hash Checker replaces that with one window: drop the file and read the answer.

### Features

- **Drag and drop** one or more files, or click to choose them.
- **Official hashing**: hashes are computed with [rcheevos](https://github.com/RetroAchievements/rcheevos), the same library RetroAchievements and its emulators use, so the result matches theirs.
- **Clear result per file**: detected console, hash (one click to copy), and status:
  - ✅ registered on RetroAchievements, with a link to the game page
  - 🟡 registered, but no achievements yet
  - ❌ not registered
  - ⚪ not compared (no API key or cached list yet)
- **Many systems**: Game Boy / Color / Advance, NES / Famicom Disk System, SNES, N64, Nintendo DS, Genesis / Mega Drive, Master System, Game Gear, 32X, Sega CD, Saturn, Dreamcast, PlayStation, PS2, PSP, PC Engine, Atari 2600, arcade, and every other system rcheevos supports.
- **Shared hash methods**: some consoles hash the same way (Sega CD and Saturn, or `.bin` cartridges for Mega Drive, 32X, Atari 2600…). The app searches each console's list and picks the one that matches. A console selector lets you override the result.
- **`.zip` support** for cartridge ROMs (one row per ROM) and for arcade sets, which RetroAchievements identifies by the zip's name.
- **Works offline** after the first download. Each console's hash list is cached and refreshed every 7 days, and Settings has a manual refresh button.
- **Private by design**: your game files never leave your computer. The app only sends read requests to the RetroAchievements Web API.
- **English and Spanish** interface, chosen from your system language.

### Download

Get the Windows installer from the **[latest release](https://github.com/JBossa/ra-hash-checker/releases/latest)**. Both formats install the same app:

- `RA-Hash-Checker_x.y.z_x64-setup.exe`: the regular installer.
- `RA-Hash-Checker_x.y.z_x64_en-US.msi`: an MSI package.

The installers aren't code-signed yet, so Windows SmartScreen may warn you the first time. Click **More info → Run anyway**.

### Getting started

1. Install the app from the [releases page](https://github.com/JBossa/ra-hash-checker/releases/latest), or build it from source (see below).
2. Get your free **Web API key**: log in to [retroachievements.org](https://retroachievements.org), open the [control panel](https://retroachievements.org/controlpanel.php) and copy the key from **Keys**.
3. Open the app, click ⚙ **Settings**, paste the key and press Enter. The key is stored in your operating system's keyring, never in a plain file.
4. Drop your games into the window.

Without a key the app still computes the hash, but it can't tell you whether that hash is registered.

### Tested systems

GBA, GB/GBC, NES, SNES, Genesis, N64 and PS1 (cue/bin) have automated tests with synthetic files, and the PS1 hash was also verified with a real disc image. The other systems rcheevos supports are expected to work, but they have no dedicated tests.

### Not supported (yet)

- CHD images (extract them to `.cue`/`.bin` with `chdman extractcd`), `.7z` / `.rar` archives (extract them first) and encrypted 3DS games.
- Compressed disc images such as `.rvz`, `.wbfs`, `.gcz`, `.wia` or `.cso`: convert them to `.iso` first (for GameCube and Wii, Dolphin can do it: right-click the game › Convert File). The app's **Supported formats** button lists every console and extension.
- Renaming, organizing, patching or converting ROMs.
- Scanning a whole library folder.

### Build from source

Requirements: stable Rust (MSVC toolchain on Windows, with Visual Studio C++ Build Tools) and Node.js 24.

```bash
git clone --recursive https://github.com/JBossa/ra-hash-checker
cd ra-hash-checker
# if you cloned without --recursive:
git submodule update --init
npm ci
npm run tauri dev     # run in development mode
npm run tauri build   # build the Windows installer (.msi / .exe)
```

Run the tests with `cargo test --workspace` and `npm test`.

Windows is the main target. macOS and Linux should build, but they haven't been tested.

### Tech stack

Rust · [Tauri 2](https://tauri.app) · TypeScript · Vite · [rcheevos](https://github.com/RetroAchievements/rcheevos) (C, compiled through a small Rust wrapper) · Vitest.

---

## Español

### Para qué sirve

RetroAchievements solo desbloquea logros si tu ROM o imagen de disco es una de las versiones que admite. Averiguar cuál tienes suele significar usar herramientas de línea de comandos o revisar tablas de hashes a mano. RA Hash Checker lo resuelve con una sola ventana: sueltas el archivo y lees la respuesta.

### Funciones

- **Arrastra y suelta** uno o varios archivos, o haz clic para elegirlos.
- **Hash oficial**: se calcula con [rcheevos](https://github.com/RetroAchievements/rcheevos), la misma librería que usan RetroAchievements y sus emuladores, así que el resultado coincide con el de ellos.
- **Resultado claro por archivo**: consola detectada, hash (se copia con un clic) y estado:
  - ✅ registrado en RetroAchievements, con enlace a la página del juego
  - 🟡 registrado, pero sin logros todavía
  - ❌ no registrado
  - ⚪ sin comparar (todavía no hay clave de API ni lista en caché)
- **Muchas consolas**: Game Boy / Color / Advance, NES / Famicom Disk System, SNES, N64, Nintendo DS, Genesis / Mega Drive, Master System, Game Gear, 32X, Sega CD, Saturn, Dreamcast, PlayStation, PS2, PSP, PC Engine, Atari 2600, arcade y todas las demás que soporta rcheevos.
- **Consolas con el mismo método de hash**: algunas calculan el hash igual (Sega CD y Saturn, o los cartuchos `.bin` de Mega Drive, 32X, Atari 2600…). La app busca en la lista de cada una y se queda con la que coincide. Con el selector de consola puedes cambiar el resultado.
- **Archivos `.zip`** de cartuchos (una fila por cada ROM) y sets de arcade, que RetroAchievements identifica por el nombre del zip.
- **Funciona sin conexión** tras la primera descarga. La lista de hashes de cada consola se guarda en caché y se renueva cada 7 días, y en Ajustes hay un botón para actualizarla a mano.
- **Privada**: tus archivos nunca salen de tu equipo. La app solo hace peticiones de lectura a la API web de RetroAchievements.
- Interfaz en **español e inglés**, según el idioma de tu sistema.

### Descarga

Descarga el instalador para Windows desde la **[última versión publicada](https://github.com/JBossa/ra-hash-checker/releases/latest)**. Los dos formatos instalan la misma app:

- `RA-Hash-Checker_x.y.z_x64-setup.exe`: el instalador normal.
- `RA-Hash-Checker_x.y.z_x64_en-US.msi`: un paquete MSI.

Los instaladores todavía no están firmados, así que Windows SmartScreen puede mostrar una advertencia la primera vez. Pulsa **Más información → Ejecutar de todas formas**.

### Primeros pasos

1. Instala la app desde la [página de versiones](https://github.com/JBossa/ra-hash-checker/releases/latest) o compílala desde el código fuente (ver abajo).
2. Consigue tu **clave de API web**, que es gratuita: entra en [retroachievements.org](https://retroachievements.org), abre el [panel de control](https://retroachievements.org/controlpanel.php) y copia la clave de **Keys**.
3. Abre la app, pulsa ⚙ **Ajustes**, pega la clave y pulsa Enter. La clave se guarda en el llavero del sistema operativo, nunca en un archivo de texto.
4. Suelta tus juegos en la ventana.

Sin clave, la app igual calcula el hash, pero no puede decirte si está registrado.

### Consolas con pruebas

GBA, GB/GBC, NES, SNES, Genesis, N64 y PS1 (cue/bin) tienen pruebas automáticas con archivos sintéticos, y el hash de PS1 se verificó además con una imagen de disco real. Las demás consolas que soporta rcheevos deberían funcionar, pero no tienen pruebas propias.

### Aún no soportado

- Imágenes CHD (extráelas a `.cue`/`.bin` con `chdman extractcd`), archivos `.7z` / `.rar` (descomprímelos primero) y juegos de 3DS cifrados.
- Imágenes de disco comprimidas como `.rvz`, `.wbfs`, `.gcz`, `.wia` o `.cso`: conviértelas antes a `.iso` (para GameCube y Wii, Dolphin lo hace: clic derecho en el juego › Convertir archivo). El botón **Formatos compatibles** de la app muestra todas las consolas y extensiones.
- Renombrar, organizar, parchear o convertir ROMs.
- Revisar una carpeta completa de juegos.

### Compilar desde el código fuente

Requisitos: Rust estable (en Windows, toolchain MSVC con las Build Tools de C++ de Visual Studio) y Node.js 24.

```bash
git clone --recursive https://github.com/JBossa/ra-hash-checker
cd ra-hash-checker
# si clonaste sin --recursive:
git submodule update --init
npm ci
npm run tauri dev     # modo desarrollo
npm run tauri build   # genera el instalador de Windows (.msi / .exe)
```

Las pruebas se ejecutan con `cargo test --workspace` y `npm test`.

Windows es la plataforma principal. Mac y Linux deberían compilar, pero no se han probado.

---

## Credits and acknowledgements / Créditos y agradecimientos

**RA Hash Checker was created by [Juan Bossa](https://github.com/JBossa).**

This project stands on the work of the **[RetroAchievements](https://retroachievements.org)** community. Thank you for keeping retro gaming alive:

- **[rcheevos](https://github.com/RetroAchievements/rcheevos)**, Copyright (c) 2018 RetroAchievements.org, MIT License. All hash calculation is done by rcheevos. Its full license is in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
- **[RetroAchievements Web API](https://api-docs.retroachievements.org)**. Game titles, hash lists and game pages come from RetroAchievements. Following RetroAchievements' guidelines for third-party tools, the app:
  - uses each user's own API key, with no shared keys;
  - caches data locally to keep requests low;
  - identifies itself with a descriptive User-Agent;
  - never re-hosts or redistributes RetroAchievements data.
- The [game identification](https://docs.retroachievements.org/developer-docs/game-identification.html) documentation explains how each system is hashed.

> **Disclaimer:** RA Hash Checker is an independent, unofficial project. It is not affiliated with, endorsed by or sponsored by RetroAchievements.org. "RetroAchievements" and all game, console and company names are trademarks of their respective owners and are used here only for identification. Please follow the [RetroAchievements Terms of Use](https://retroachievements.org/terms) when using your API key.
>
> **Aviso:** RA Hash Checker es un proyecto independiente y no oficial. No está afiliado a RetroAchievements.org ni cuenta con su respaldo o patrocinio. "RetroAchievements" y los nombres de juegos, consolas y empresas son marcas de sus respectivos dueños y aquí solo se usan para identificarlos.

## License / Licencia

[MIT](LICENSE) © 2026 Juan Bossa. Third-party components keep their own licenses; see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
