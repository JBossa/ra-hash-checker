# Soporte de `.wbfs` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Calcular el hash de Wii de un `.wbfs` sin convertirlo.

**Architecture:** Módulo `wbfs` en `crates/rcheevos-hash` con un lector que presenta el `.wbfs` como el ISO; `hash()` instala sus callbacks en el iterador cuando la extensión es `.wbfs`. La app solo lo registra como Wii.

**Tech Stack:** Rust (FFI con rcheevos v12.5.0), Tauri, TypeScript.

**Spec:** `docs/superpowers/specs/2026-11-22-wbfs-support-design.md`

## Global Constraints
- Sin dependencias nuevas.
- Nada de pánicos a través de FFI; errores de E/S = ceros / `NULL`.
- El `.wbfs` nunca se carga entero en memoria.

## Review Focus
- `.wbfs` dividido (`.wbf1`): una lectura que cruza el límite entre partes devuelve los bytes correctos.
- Bloque con `wlba == 0`: se lee como ceros.
- Archivo que no es WBFS (magia incorrecta) con extensión `.wbfs`: `hash` devuelve `None`, sin colgarse.
- Mayúsculas en la extensión (`.WBFS`).
- `.wbfs` dentro de un `.zip`: aviso de disco en zip.

---

### Task 1: Lector WBFS en el crate

**Files:** Create `crates/rcheevos-hash/src/wbfs.rs`, `crates/rcheevos-hash/tests/wbfs.rs`; Modify `src/lib.rs`, `src/expansions.rs`, `src/formats.rs`

**Interfaces — Produces:**
- `wbfs::WbfsReader::open(path: &Path) -> Option<WbfsReader>`, `read_at(&mut self, pos: u64, buf: &mut [u8])`.
- `hash(path, None, 19)` funciona con `.wbfs`; `candidates("x.wbfs")` = `[Wii(19)]`.

- [ ] Tests en `tests/wbfs.rs`: ISO sintético de Wii (cabecera, tabla de particiones, TMD, datos) y su `.wbfs` construido en el test → `hash(iso, None, 19) == hash(wbfs, None, 19)`; igual con el `.wbfs` dividido en `.wbfs` + `.wbf1`; `.WBFS` en mayúsculas; magia incorrecta → `None`.
- [ ] Implementar; `cargo test -p rcheevos-hash` en verde.

### Task 2: App

**Files:** `src-tauri/src/checker.rs`, `src-tauri/src/inputs.rs`, `src/i18n.ts`, `README.md`

- [ ] Test: `.wbfs` ya no da `CompressedDisc`; `.wbfs` en zip → `disc_in_zip`.
- [ ] Quitar `wbfs` de `COMPRESSED_DISCS`, añadirlo a `DISC_EXTENSIONS`; textos sin `.wbfs` en la lista de comprimidos; README.
- [ ] `cargo test --workspace`, `npm test`, `npm run build`; commit.
