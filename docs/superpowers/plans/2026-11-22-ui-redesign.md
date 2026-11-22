# Rediseño de la UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rediseñar la UI con estilo RetroAchievements y mostrar una ficha completa del juego al coincidir.

**Architecture:** El backend añade `gameHashes` a `CheckResult` y un comando `game_info(gameId)` cacheado en disco. El frontend (TS + DOM, sin framework) reescribe `view.ts` (tarjetas y ficha), `main.ts` (estado del historial, filtros) y `styles.css` (tokens de tema).

**Tech Stack:** Tauri 2, Rust (reqwest bloqueante, serde), TypeScript, Vite, Vitest + jsdom.

**Spec:** `docs/superpowers/specs/2026-11-22-ui-redesign-design.md`

## Global Constraints
- Sin frameworks de frontend nuevos.
- Imágenes: `https://media.retroachievements.org` + ruta de la API (`/Images/xxxxxx.png`).
- Textos en `i18n.ts` en es y en.
- Ventana 960×720, mínimo 640×520.
- No copiar el logo de RetroAchievements.

## Review Focus
- Juego sin portada/metadatos (campos nulos o vacíos): la ficha muestra un hueco, no `null` ni una imagen rota.
- Sin conexión o sin clave: `game_info` devuelve `null` y la ficha sigue mostrando título, hashes y enlace.
- Hashes del juego no disponibles (`gameHashes` vacío): la sección no aparece, sin errores.
- Cientos de archivos: los filtros y contadores se mantienen correctos al cambiar de consola en una fila.
- Mayúsculas/minúsculas: el hash del usuario se resalta en la lista aunque cambie el caso.

---

### Task 1: Backend — `gameHashes` y `game_info`

**Files:** Modify `src-tauri/src/model.rs`, `ra_api.rs`, `cache.rs`, `source.rs`, `checker.rs`, `lib.rs`

**Interfaces — Produces:**
- `CheckResult.game_hashes: Vec<GameHash>` (JSON `gameHashes`, objetos `{MD5, Name, Labels}`).
- `GameInfo` (PascalCase, todos `Option<String>` con `#[serde(default)]`): `ImageIcon, ImageTitle, ImageIngame, ImageBoxArt, Publisher, Developer, Genre, Released`.
- `RaClient::get_game(game_id) -> Result<GameInfo, RaError>` (`API_GetGame.php?i=`).
- `HashCache::{load_game_info, save_game_info}` en `info/{id}.json`.
- `source::fetch_game_info(cache, client: Option<&RaClient>, game_id) -> Option<GameInfo>`.
- Comando `game_info(gameId: u32) -> Option<GameInfo>`.

- [ ] Test `matched_result_includes_all_game_hashes` en checker.rs: con los 2 hashes de `matched_name_from_game_hashes`, `r.game_hashes.len() == 2`.
- [ ] Test `game_parses_info` en ra_api.rs (mockito, cuerpo con `ImageBoxArt:"/Images/1.png"`, `Publisher:"Nintendo"` y campos extra) → `box_art == Some("/Images/1.png")`.
- [ ] Test `roundtrip_game_info` en cache.rs.
- [ ] Implementar; `cargo test` en verde; commit.

### Task 2: Frontend — vista, historial y ficha

**Files:** Rewrite `src/view.ts`, `src/main.ts`, `src/styles.css`, `index.html`; Modify `src/i18n.ts`, `src/view.test.ts`

**Interfaces — Produces (view.ts):**
- `renderRow(r: CheckResult, t: Translator): HTMLElement` (sigue existiendo; ahora una tarjeta).
- `fillGameInfo(card: HTMLElement, info: GameInfo | null, t: Translator): void`.
- `statusGroup(s: Status): 'ok' | 'missing' | 'other'`.
- `MEDIA_URL = 'https://media.retroachievements.org'`.

- [ ] Tests en view.test.ts: tarjeta registrada lista todos los `gameHashes` y marca con `.is-match` solo el que coincide sin importar mayúsculas; `fillGameInfo(card, null)` deja el hueco de portada sin `<img>`; `fillGameInfo` con `ImageBoxArt` crea `img[src="https://media.retroachievements.org/Images/1.png"]`; `statusGroup('registeredNoAchievements') === 'ok'`, `statusGroup('unsupported') === 'other'`.
- [ ] Implementar; `npm test` y `npm run build` en verde; commit.

### Task 3: Ajustes, logo e iconos, ventana

**Files:** `index.html`, `src/styles.css`, `src/main.ts`, `src/logo.svg` (nuevo), `src-tauri/icons/*` (regenerados con `npx tauri icon`), `src-tauri/tauri.conf.json`

- [ ] Panel de ajustes con secciones; mostrar/ocultar clave; enlace para obtenerla.
- [ ] Logo SVG original; regenerar iconos.
- [ ] Ventana 960×720 / mín. 640×520.
- [ ] `npm run build`, `cargo build`, revisar en la app; commit.
