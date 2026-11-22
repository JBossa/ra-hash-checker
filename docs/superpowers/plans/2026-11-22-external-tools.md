# Herramientas externas (Dolphin) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Comprobar `.rvz`, `.gcz` y `.wia` convirtiéndolos con el DolphinTool del usuario.

**Architecture:** Módulo `src-tauri/src/tools/` (especificación de herramientas, configuración, detección, cabeceras y conversión); `lib.rs` separa los archivos de herramienta en una cola de un hilo; el frontend añade la sección de ajustes, tarjetas provisionales y marcas en formatos.

**Tech Stack:** Rust (Tauri 2), `windows-sys` (espacio libre en Windows), TypeScript.

**Spec:** `docs/superpowers/specs/2026-11-22-external-tools-design.md`

## Global Constraints
- Sin shell: `std::process::Command` con argumentos en vector; `CREATE_NO_WINDOW` en Windows.
- El `.iso` temporal y la carpeta `user` se borran siempre; `tmp` se vacía al arrancar.
- Un solo `convert` a la vez en toda la app.
- Commits a nombre del usuario, sin coautoría.

## Review Focus
- Ruta guardada que ya no existe → «no encontrada» y `NeedsTool` al soltar.
- Espacio insuficiente → `NoDiskSpace` sin lanzar Dolphin.
- DolphinTool termina con error → `ToolFailed` y el temporal no queda en disco.
- Soltar 3 `.rvz` y 2 ROMs: las ROMs salen sin esperar; los `.rvz` pasan de «En cola» a «Convirtiendo» a resultado.
- Cabecera de RVZ/WIA/GCZ truncada o con magia incorrecta → `output_size` = `None` (se convierte igualmente si el
  espacio libre supera el máximo de Wii, 8,5 GB; si no, `NoDiskSpace`).

---

### Task 1: Módulo `tools` (sin UI)
**Files:** Create `src-tauri/src/tools/mod.rs`; Modify `src-tauri/Cargo.toml`, `src-tauri/src/model.rs`
**Produces:** `ToolSpec`, `TOOLS`, `tool_for_extension(ext) -> Option<&ToolSpec>`, `ToolsConfig::{load, save, path_for, set, clear}`,
`output_size(path) -> Option<u64>`, `validate(spec, exe) -> bool`, `detect(spec) -> Option<PathBuf>`,
`convert(spec, exe, input, tmp) -> Result<PathBuf, ToolError>`, `free_space(dir) -> Option<u64>`.
- [ ] Tests: cabeceras RVZ/WIA/GCZ sintéticas → tamaño; magia mala → `None`; config ida y vuelta; ruta inexistente → «no encontrada»; `validate` con un ejecutable que no es Dolphin → `false`.
- [ ] Implementar; `cargo test`; commit.

### Task 2: Integración en la comprobación
**Files:** `src-tauri/src/lib.rs`, `src-tauri/src/checker.rs`, `src-tauri/src/model.rs`
- [ ] `checker::check_only(input, consoles, src)`; test: una imagen solo se compara con las consolas dadas y sin alternativas.
- [ ] `MessageCode::{NeedsTool, NoDiskSpace, ToolFailed}`, `CheckResult.tool: Option<String>`; `early_result` deja de devolver `CompressedDisc` para extensiones de herramienta.
- [ ] Cola de conversiones y comandos `get_tools`, `detect_tool`, `set_tool_path`, `clear_tool`; evento `progress`.
- [ ] `cargo test`; commit.

### Task 3: UI
**Files:** `index.html`, `src/main.ts`, `src/view.ts`, `src/i18n.ts`, `src/styles.css`, `src/formats.ts`, `src/tools.ts` (nuevo), tests
- [ ] Tests: `renderToolCard` por estado; tarjeta provisional por etapa; `needsTool` con botón; formatos con marca de herramienta.
- [ ] Implementar; `npm test`, `npm run build`; commit.

### Task 4: Prueba real
- [ ] Conectar el DolphinTool de `Downloads`, soltar el `.rvz` de Super Mario Galaxy → «Registrado» (`4e0d0d2f…`); temporales borrados.
