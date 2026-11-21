# RA Hash Checker: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Una app de escritorio de una sola pantalla. El usuario suelta archivos de juegos; la app calcula su hash de RetroAchievements con rcheevos y dice si ese hash está registrado en RA.

**Architecture:**
- Un workspace de Cargo con dos piezas:
  - El crate `rcheevos-hash`, que compila la parte de hashes de rcheevos (C) y la expone con una API segura en Rust.
  - El backend de Tauri 2 (`src-tauri`). Este orquesta: lee `.zip`, prueba las consolas candidatas, consulta las listas de hashes de RA (cacheadas en disco) y devuelve un resultado por archivo.
- El frontend es TypeScript + Vite, sin framework.

**Tech Stack:** Rust (stable, MSVC), Tauri 2, rcheevos v12.5.0 (submódulo git), crates `cc`, `reqwest` (blocking, rustls), `serde`, `zip`, `keyring`, `tempfile`, `md5` y `mockito` (solo para tests); TypeScript, Vite y Vitest.

**Spec:** `docs/superpowers/specs/2026-11-21-ra-hash-checker-design.md`

## Global Constraints

- **rcheevos:**
  - Se fija el tag `v12.5.0`. En ese tag el código de hashes está en `src/rhash/` (no en `src/hash/`).
  - Se compila con `RC_HASH_NO_ENCRYPTED` definido.
  - Se usa la API de iterador: `rc_hash_initialize_iterator`, `rc_hash_generate` y `rc_hash_destroy_iterator`. Nada de las funciones marcadas `[deprecated]`.
- **Licencia del proyecto:** MIT.
- **Endpoints de RA permitidos** (solo estos dos):
  - `https://retroachievements.org/API/API_GetGameList.php?i=<consola>&h=1&y=<clave>`, **sin `f=1`**.
  - `https://retroachievements.org/API/API_GetGameHashes.php?i=<gameId>&y=<clave>`.
  - Nunca `dorequest.php`.
  - Nunca una clave compartida o incluida en la app.
- **User-Agent:** `RAHashChecker/<CARGO_PKG_VERSION> (+https://github.com/<owner>/ra-hash-checker)`. `<owner>` queda como constante `REPO_URL` en `ra_api.rs` hasta que exista el repo en GitHub.
- **Caché:** una lista por consola, renovada si tiene **más de 7 días**. La clave de API se guarda en el llavero del sistema (servicio `ra-hash-checker`, usuario `api-key`), nunca en un archivo.
- **Estados de resultado:**
  - `Registered`: ✅ Registrado.
  - `RegisteredNoAchievements`: 🟡 Registrado, sin logros todavía (`NumAchievements == 0`).
  - `NotRegistered`: ❌ No registrado.
  - `Unchecked`: ⚪ Sin comparar.
  - Dos estados de error, sin ícono definido en el spec: `Unsupported` y `Error`.
- **`.zip`:** solo para cartuchos. Cada ROM del zip produce una fila. Si el zip contiene un disco (`.cue`, `.bin`, `.iso`, `.gdi`, `.chd`, `.m3u`), esa fila sale con el mensaje "Descomprime el disco y suéltalo de nuevo".
- **Consolas ambiguas:** se prueban las candidatas en el orden de rcheevos. La lista de cada consola se descarga solo cuando le toca a esa candidata, y la búsqueda se detiene en la primera coincidencia.
- **Interfaz:**
  - Una sola pantalla más un diálogo de ajustes.
  - Idioma español o inglés según `navigator.language`, con inglés como respaldo.
  - Sin la sugerencia "¿Quizás…?", que queda para una versión futura.
- **Decisión de este plan:** el campo "usuario de RA" del spec se omite, porque los endpoints solo necesitan la clave. Ajustes = clave de API + actualizar listas + fecha de última actualización.

## Review Focus

1. **Rutas con acentos, espacios o paréntesis** (por ejemplo `Pokémon Esmeralda (España).gba`): el hash debe salir igual que con una ruta ASCII. → Test `hash_non_ascii_path` en la Task 1.
2. **Archivos que no son juegos o extensiones desconocidas** (`.txt`, una carpeta, un `.bin` suelto que no es de ninguna consola): una fila `Unsupported`/`Error` con mensaje, nunca un pánico ni la pérdida de las demás filas. → Tests `unknown_extension_is_unsupported` y `directory_is_error` en la Task 4.
3. **Archivos enormes** (un `.bin` de PS1 de 550 MB o una ISO de PS2 de 4 GB): la ventana no se congela y cada fila aparece cuando termina su archivo. → El comando usa `tauri::async_runtime::spawn_blocking` y emite un evento por archivo (Task 6). La verificación manual con el `.cue` de PS1 va en la Task 7.
4. **Clave inválida, sin internet o caché vieja:**
   - Clave inválida (401) → `Unchecked` con el mensaje `InvalidApiKey`.
   - Sin internet → se usa la caché vieja si existe; si no hay caché, `Unchecked` con el mensaje `Offline`.
   - → Tests en la Task 5.
5. **MD5 de RA en mayúsculas o minúsculas:** la comparación no distingue mayúsculas. → Test `lookup_is_case_insensitive` en la Task 3.

---

## Estructura de archivos

```
Cargo.toml                         workspace: members = ["crates/rcheevos-hash", "src-tauri"]
LICENSE  README.md  .gitignore  .gitmodules
crates/rcheevos-hash/
  Cargo.toml  build.rs
  vendor/rcheevos/                 submódulo git @ v12.5.0
  src/lib.rs                       FFI + API segura (candidate_consoles, hash)
  src/consoles.rs                  console_name(id)
  tests/hash.rs
src-tauri/
  Cargo.toml  build.rs  tauri.conf.json  capabilities/default.json
  src/main.rs                      arranque
  src/lib.rs                       comandos Tauri + estado
  src/model.rs                     tipos compartidos (GameEntry, CheckResult, Status, MessageCode)
  src/ra_api.rs                    cliente HTTP de RA
  src/cache.rs                     caché en disco por consola
  src/inputs.rs                    ruta → entradas (zip incluido)
  src/checker.rs                   orquestación por archivo
  src/source.rs                    HashListSource real (caché + API + llavero)
index.html  package.json  vite.config.ts  tsconfig.json
src/main.ts  src/view.ts  src/i18n.ts  src/styles.css
src/view.test.ts  src/i18n.test.ts
.github/workflows/ci.yml
```

---

### Task 0: Herramientas de compilación

> Instala software pesado (unos 6 GB en total). **Pide confirmación al usuario antes de cada instalación.**

**Files:** ninguno.

- [ ] **Step 1: Instalar Rust**

  Run: `winget install -e --id Rustlang.Rustup`, y luego, en una terminal nueva, `rustup default stable-x86_64-pc-windows-msvc`.
  Expected: `cargo --version` imprime `cargo 1.x`.

- [ ] **Step 2: Instalar Visual Studio Build Tools con C++**

  Run: `winget install -e --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`
  Expected: `vswhere -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath` devuelve una ruta.

- [ ] **Step 3: Comprobar el resto**

  Node v24 y WebView2 ya están instalados (verificado). Run: `node --version`. Expected: `v24.x`.

---

### Task 1: Crate `rcheevos-hash`

**Files:**
- Create:
  - `Cargo.toml` (workspace)
  - `.gitignore` (`target/`, `node_modules/`, `dist/`)
  - `.gitmodules`
  - `crates/rcheevos-hash/{Cargo.toml,build.rs,src/lib.rs,src/consoles.rs}`
- Test: `crates/rcheevos-hash/tests/hash.rs`

**Interfaces:**
- Produces:
  - `pub fn candidate_consoles(path: &Path, data: Option<&[u8]>) -> Vec<u32>`: los `consoles[]` del iterador hasta el primer 0, en el orden de rcheevos. Vacío si la extensión no se reconoce.
  - `pub fn hash(path: &Path, data: Option<&[u8]>, console_id: u32) -> Option<String>`: MD5 en minúsculas, 32 caracteres; `None` si rcheevos devuelve 0.
  - `pub fn console_name(id: u32) -> &'static str`: nombre legible, o `"Unknown"`.
  - Si `data` es `Some`, `path` solo aporta el nombre de archivo (caso zip).
  - Ambas funciones crean y destruyen su propio iterador, sin estado global, así que se pueden llamar desde varios hilos.

- [ ] **Step 1: Añadir rcheevos como submódulo fijado**

  Run: `git submodule add https://github.com/RetroAchievements/rcheevos crates/rcheevos-hash/vendor/rcheevos`, y luego, dentro del submódulo, `git checkout v12.5.0`.
  Expected: existe `crates/rcheevos-hash/vendor/rcheevos/src/rhash/hash.c`.

- [ ] **Step 2: Escribir los tests que fallan** en `tests/hash.rs`

  Las ROMs se generan en el test, en un `tempfile::tempdir()`. El hash esperado se calcula con el crate `md5` sobre los bytes que RA hashea.

  ```rust
  fn md5_hex(b: &[u8]) -> String { format!("{:x}", md5::compute(b)) }

  #[test] fn gba_is_md5_of_whole_file()      // 4096 bytes pseudoaleatorios en x.gba → hash(path,None,5) == md5_hex(all)
  #[test] fn gba_candidates()                // candidate_consoles("x.gba", None) == vec![5]
  #[test] fn gb_gbc_genesis_whole_file()     // mismos bytes en x.gb (4), x.gbc (6), x.md (1) → cada hash == md5_hex(all); candidate_consoles de cada uno contiene ese id
  #[test] fn nes_skips_ines_header()         // b"NES\x1a" + 12 bytes + 32768 data → hash(..,7) == md5_hex(data)
  #[test] fn snes_skips_copier_header()      // 512 bytes header + 8192*4 data (len % 8192 == 512) → hash(..,3) == md5_hex(data)
  #[test] fn n64_v64_equals_z64()            // z64: empieza 0x80,0x37,0x12,0x40; v64 = mismo con cada par de bytes intercambiado → hash(v64,2) == hash(z64,2)
  #[test] fn buffer_input_matches_file()     // hash("inner.gba", Some(&bytes), 5) == hash(file_with_same_bytes, None, 5)
  #[test] fn hash_non_ascii_path()           // archivo "Pokémon Esmeralda (España).gba" → mismo hash que "plain.gba" con los mismos bytes
  #[test] fn unknown_extension_has_no_candidates() // "notes.txt" → vec![]
  #[test] fn console_names()                 // console_name(5)=="Game Boy Advance", console_name(12)=="PlayStation", console_name(9999)=="Unknown"
  #[test] #[ignore] fn ps1_bloody_roar_2()   // ruta en env RA_PS1_CUE → candidate_consoles contiene 12 y hash(..,12) == "77defab4cedf9f82c4251a474d55cbee"
  ```

- [ ] **Step 3: Ejecutar los tests y confirmar que fallan**

  Run: `cargo test -p rcheevos-hash`
  Expected: FAIL (no compila: funciones no definidas).

- [ ] **Step 4: Escribir `build.rs`**

  - Con el crate `cc`, compilar desde `vendor/rcheevos`:
    - `src/rhash/{hash.c,hash_rom.c,hash_disc.c,hash_zip.c,cdreader.c,md5.c}`
    - `src/{rc_compat.c,rc_util.c,rc_version.c}`
  - Includes: `include/` y `src/`.
  - Define: `RC_HASH_NO_ENCRYPTED`.
  - Emitir `cargo:rerun-if-changed=vendor/rcheevos/src/rhash`.
  - Si el enlazado falla por un símbolo sin resolver, añadir el `.c` de `src/` que lo define. No añadir nada de `src/rcheevos/`, `src/rapi/` ni `rc_client*`.

- [ ] **Step 5: Implementar las declaraciones FFI y la API segura en `src/lib.rs`**

  - **Declaraciones FFI:** se declaran a mano con `#[repr(C)]`, copiando `rc_hash_iterator_t` y `rc_hash_callbacks_t` del `include/rc_hash.h` de v12.5.0:
    - `rc_hash_callbacks_t` con `RC_HASH_NO_ENCRYPTED` definido, es decir, sin el bloque `encryption`. Contiene `verbose_message`, `error_message`, `filereader` (5 punteros) y `cdreader` (4 punteros).
    - `rc_hash_iterator_t`: `buffer`, `buffer_size`, `consoles: [u8; 12]`, `index: i32`, `path`, `userdata` y `callbacks`.
  - Se añade un test `layout_matches` que compara `size_of::<RcHashIterator>()` con un `sizeof` exportado por un `.c` auxiliar de una línea compilado en `build.rs`. Así se evita un desajuste silencioso del struct.
  - **Ruta:** a `CString` desde `path.to_str()`. Si la ruta no es UTF-8, devuelve vacío o `None`.
  - **`console_name`:** tabla `match` en `src/consoles.rs` con los IDs 1 a 82 de `include/rc_consoles.h`. Nombres para mostrar: `1 "Genesis/Mega Drive"`, `5 "Game Boy Advance"`, `12 "PlayStation"`, etc.

- [ ] **Step 6: Ejecutar los tests y confirmar que pasan**

  Run: `cargo test -p rcheevos-hash`
  Expected: PASS (todos menos el `#[ignore]`).

- [ ] **Step 7: Ejecutar el test de PS1 con la imagen local**

  Run: `$env:RA_PS1_CUE="C:\Users\JuanBo\Downloads\Bloody Roar II (USA)\Bloody Roar II (USA)\Bloody Roar II (USA).cue"; cargo test -p rcheevos-hash -- --ignored`
  Expected: PASS.

- [ ] **Step 8: Commit**

  ```bash
  git add .gitignore .gitmodules Cargo.toml crates/
  git commit -m "feat: crate rcheevos-hash con la API de hashes de rcheevos v12.5.0"
  ```

---

### Task 2: Tipos compartidos y cliente de la API de RA

**Files:**
- Create: los esqueletos de `src-tauri/` generados con `npm create tauri-app@latest` (plantilla `vanilla-ts`, gestor `npm`, identificador `io.github.rahashchecker`, nombre de producto `RA Hash Checker`). Se mueven a la raíz del repo y `src-tauri` se añade al workspace.
- Create: `src-tauri/src/model.rs` y `src-tauri/src/ra_api.rs`.
- Test: módulo `#[cfg(test)]` en `ra_api.rs`, con `mockito`.

**Interfaces:**
- Produces (`model.rs`, todos `Serialize`/`Deserialize`/`Clone`/`Debug`; los JSON de RA usan PascalCase):
  - `GameEntry { id: u32, title: String, console_id: u32, num_achievements: u32, hashes: Vec<String> }`, que mapea `ID`, `Title`, `ConsoleID`, `NumAchievements` y `Hashes`.
  - `GameHash { md5: String, name: String, labels: Vec<String> }`, que mapea `MD5`, `Name` y `Labels` (la clave raíz es `Results`).
  - `Status { Registered, RegisteredNoAchievements, NotRegistered, Unchecked, Unsupported, Error }`, serializado en camelCase.
  - `MessageCode { DiscInZip, NoHash, ReadError, InvalidApiKey, Offline, NoApiKey }`, serializado en camelCase.
  - `CheckResult { file_name: String, source_path: String, inner_name: Option<String>, console_id: Option<u32>, console_name: Option<String>, md5: Option<String>, status: Status, game: Option<GameEntry>, matched_name: Option<String>, alternatives: Vec<(u32, String)>, message: Option<MessageCode> }`, serializado en camelCase hacia el frontend.
    - `alternatives` lista `(console_id, console_name)` para el selector.
- Produces (`ra_api.rs`):
  - `pub enum RaError { Unauthorized, Network(String), Parse(String) }`
  - `pub struct RaClient`
  - `RaClient::new(base_url: &str, api_key: String) -> RaClient`. La base de producción es `https://retroachievements.org/API`.
  - `fn get_game_list(&self, console_id: u32) -> Result<Vec<GameEntry>, RaError>`
  - `fn get_game_hashes(&self, game_id: u32) -> Result<Vec<GameHash>, RaError>`

- [ ] **Step 1: Generar y mover el esqueleto de Tauri**

  Comprobar que `npm install` funciona y que `cargo build -p ra-hash-checker` compila.

- [ ] **Step 2: Escribir los tests que fallan**

  ```rust
  #[test] fn game_list_parses_and_sends_params()   // mock GET /API_GetGameList.php con query i=5,h=1,y=KEY y SIN f; header User-Agent empieza "RAHashChecker/"; body [{"ID":1,"Title":"T","ConsoleID":5,"NumAchievements":3,"Hashes":["ABC"]}] → vec con id 1, hashes ["ABC"]
  #[test] fn game_hashes_parses_results()           // body {"Results":[{"MD5":"abc","Name":"T (USA)","Labels":["nointro"],"PatchUrl":null}]} → name "T (USA)"
  #[test] fn unauthorized_maps_to_error()           // status 401 → Err(RaError::Unauthorized)
  #[test] fn garbage_body_is_parse_error()          // body "<html>" → Err(RaError::Parse(_))
  ```

- [ ] **Step 3: Ejecutar los tests y confirmar que fallan**

  Run: `cargo test -p ra-hash-checker ra_api`
  Expected: FAIL.

- [ ] **Step 4: Implementar `model.rs` y `ra_api.rs`**

  Usar `reqwest::blocking` con rustls y un timeout de 60 s, porque las listas pueden ser grandes. Los errores de conexión o timeout devuelven `Network`.

- [ ] **Step 5: Ejecutar los tests y confirmar que pasan**

  Run: `cargo test -p ra-hash-checker ra_api`
  Expected: PASS.

- [ ] **Step 6: Commit**

  ```bash
  git commit -am "feat: esqueleto Tauri y cliente de la API de RA"
  ```

---

### Task 3: Caché en disco

**Files:**
- Create: `src-tauri/src/cache.rs`
- Test: módulo `#[cfg(test)]` en el mismo archivo, con `tempfile`.

**Interfaces:**
- Consumes: `GameEntry` y `GameHash` (Task 2).
- Produces:
  - `pub struct CachedList { pub fetched_at: u64 /* unix s */, pub games: Vec<GameEntry> }`
  - `pub struct HashCache`
  - `HashCache::new(dir: PathBuf) -> HashCache`
  - `fn load_list(&self, console_id: u32) -> Option<CachedList>`
  - `fn save_list(&self, console_id: u32, games: &[GameEntry], now: u64) -> std::io::Result<()>`
  - `fn is_stale(list: &CachedList, now: u64) -> bool`: devuelve `true` si `now - fetched_at > 7*24*3600`.
  - `fn find(list: &CachedList, md5: &str) -> Option<&GameEntry>`: no distingue mayúsculas.
  - `fn load_game_hashes(&self, game_id: u32) -> Option<Vec<GameHash>>`
  - `fn save_game_hashes(&self, game_id: u32, hashes: &[GameHash]) -> std::io::Result<()>`
  - `fn cached_consoles(&self) -> Vec<u32>`
  - `fn last_refresh(&self) -> Option<u64>`: el `fetched_at` más reciente.
- Archivos: `lists/<console_id>.json` y `games/<game_id>.json` dentro de `dir`.

- [ ] **Step 1: Escribir los tests que fallan**

  ```rust
  #[test] fn roundtrip_list()                  // save_list(5,..,1000) → load_list(5) con fetched_at 1000 y mismos juegos
  #[test] fn missing_list_is_none()            // load_list(7) == None
  #[test] fn corrupt_file_is_none()            // escribir "{" en lists/5.json → load_list(5) == None (sin pánico)
  #[test] fn staleness_boundary()              // fetched_at 0: is_stale(now=604800)==false, is_stale(now=604801)==true
  #[test] fn lookup_is_case_insensitive()      // hashes ["ABCDEF..."] → find(list,"abcdef...") encuentra el juego
  #[test] fn cached_consoles_and_last_refresh()// listas 5 (t=10) y 12 (t=20) → cached_consoles ⊇ {5,12}, last_refresh == Some(20)
  ```

- [ ] **Step 2: Ejecutar los tests y confirmar que fallan**

  Run: `cargo test -p ra-hash-checker cache`
  Expected: FAIL.

- [ ] **Step 3: Implementar `cache.rs`**

  Usar `serde_json`. Para que un cierre a medias no corrompa la caché, se escribe primero en un archivo `.tmp` y luego se renombra.

- [ ] **Step 4: Ejecutar los tests y confirmar que pasan**

  Run: `cargo test -p ra-hash-checker cache`
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git commit -am "feat: caché de listas de hashes por consola"
  ```

---

### Task 4: Entradas (zip) y orquestación por archivo

**Files:**
- Create: `src-tauri/src/inputs.rs` y `src-tauri/src/checker.rs`
- Test: módulos `#[cfg(test)]` en ambos archivos. Los zips se crean en el test con el crate `zip`.

**Interfaces:**
- Consumes:
  - `candidate_consoles`, `hash` y `console_name` (Task 1).
  - `GameEntry`, `GameHash`, `CheckResult`, `Status` y `MessageCode` (Task 2).
- Produces (`inputs.rs`):
  - `pub struct Input { pub source_path: PathBuf, pub display_name: String, pub inner_name: Option<String>, pub data: Option<Vec<u8>>, pub disc_in_zip: bool }`
  - `pub fn expand(path: &Path) -> Result<Vec<Input>, MessageCode>`. Un archivo normal da una entrada con `data: None`. Un `.zip` da una entrada por cada archivo que no sea carpeta, con `data` en memoria. Las entradas de disco del zip llevan `disc_in_zip: true` y `data: None`. Un zip ilegible devuelve `Err(ReadError)`.
- Produces (`checker.rs`):
  - `pub enum ListOutcome { Available(Vec<GameEntry>), Unavailable(MessageCode) }`
  - `pub trait HashListSource { fn list(&self, console_id: u32) -> ListOutcome; fn game_hashes(&self, game_id: u32) -> Option<Vec<GameHash>>; }`
  - `pub fn check(input: &Input, src: &dyn HashListSource) -> CheckResult`
  - `pub fn check_as(input: &Input, console_id: u32, src: &dyn HashListSource) -> CheckResult`: para el selector de consola; hashea solo esa consola.

- [ ] **Step 1: Escribir los tests que fallan**

  Se usa un `FakeSource` que registra en un `RefCell<Vec<u32>>` las consolas que se le pidieron.

  ```rust
  #[test] fn gba_match_is_registered()            // lista 5 con el hash del archivo y NumAchievements 10 → Registered, game.id correcto, console_id Some(5)
  #[test] fn zero_achievements()                  // igual con NumAchievements 0 → RegisteredNoAchievements
  #[test] fn no_match_is_not_registered()         // lista 5 sin el hash → NotRegistered, md5 Some(..)
  #[test] fn no_list_is_unchecked()               // list(5) → Unavailable(NoApiKey) → Unchecked, message Some(NoApiKey), md5 Some(..)
  #[test] fn ambiguous_stops_at_first_match()     // archivo .bin que rcheevos hashea para varias candidatas; la fuente solo tiene coincidencia en la 2ª consola que hashea → resultado con esa consola; consolas pedidas == [1ª, 2ª] (no pidió la 3ª)
  #[test] fn ambiguous_no_match_uses_first()      // ninguna coincide → NotRegistered con console_id = primera candidata hasheada, alternatives = las demás hasheadas
  #[test] fn matched_name_from_game_hashes()      // game_hashes devuelve [{md5: <hash>, name:"Juego (USA)"}] → matched_name Some("Juego (USA)")
  #[test] fn unknown_extension_is_unsupported()   // "notes.txt" → Unsupported
  #[test] fn directory_is_error()                 // expand(dir) → Err(ReadError) → (en lib.rs) fila Error
  #[test] fn zip_two_roms_two_inputs()            // zip con a.gba y b.gb → 2 Inputs con inner_name y data
  #[test] fn zip_with_cue_flags_disc()            // zip con juego.cue + juego.bin → entradas disc_in_zip; check → Error con DiscInZip
  ```

  Para `ambiguous_*`, conviene un `.bin` de bytes arbitrarios: rcheevos lo hashea como Mega Drive, que siempre produce hash, y las candidatas de disco fallan. El test debe usar las candidatas reales que devuelva `candidate_consoles` en vez de suponerlas.

- [ ] **Step 2: Ejecutar los tests y confirmar que fallan**

  Run: `cargo test -p ra-hash-checker checker inputs`
  Expected: FAIL.

- [ ] **Step 3: Implementar `expand` y `check`**

  Algoritmo de `check` (lo que los tests no fijan):

  ```
  si input.disc_in_zip → Error + DiscInZip
  cands = candidate_consoles(path_o_nombre, data)
  si vacío → Unsupported
  hashed = []                       // (console, md5) que rcheevos pudo generar
  first_unavailable = None
  para c en cands:
      h = hash(c)?; si None → continuar
      hashed.push((c,h))
      match src.list(c):
          Available(games) → si find(games,h) → resultado Registered/RegisteredNoAchievements
                              (matched_name = src.game_hashes(id) donde md5 == h, sin distinguir mayúsculas)
          Unavailable(code) → first_unavailable.get_or_insert((c,code))
  si hashed vacío → Error + NoHash
  (c0,h0) = hashed[0]
  status = si la lista de c0 estuvo Available → NotRegistered, si no → Unchecked + su code
  alternatives = hashed[1..] (con console_name)
  ```

- [ ] **Step 4: Ejecutar los tests y confirmar que pasan**

  Run: `cargo test -p ra-hash-checker checker inputs`
  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git commit -am "feat: lectura de zip y comprobación por archivo"
  ```

---

### Task 5: Fuente real (caché + API + llavero)

**Files:**
- Create: `src-tauri/src/source.rs`
- Test: módulo `#[cfg(test)]`, con `mockito` y `tempfile`.

**Interfaces:**
- Consumes:
  - `RaClient` y `RaError` (Task 2).
  - `HashCache` (Task 3).
  - `HashListSource` y `ListOutcome` (Task 4).
- Produces:
  - `pub struct LiveSource { cache: HashCache, client: Option<RaClient>, now: u64 }`
  - `LiveSource::new(cache: HashCache, client: Option<RaClient>, now: u64)`
  - `impl HashListSource for LiveSource`
  - `pub fn refresh_all(cache: &HashCache, client: &RaClient, now: u64) -> Result<usize, RaError>`: vuelve a descargar todas las `cached_consoles()` y devuelve cuántas actualizó.
  - Llavero:
    - `pub fn load_api_key() -> Option<String>`
    - `pub fn store_api_key(key: &str) -> keyring::Result<()>` (una clave vacía borra la entrada)
- Comportamiento de `list(c)`:
  1. Caché fresca → la usa.
  2. Si no, con cliente → descarga, guarda y la usa.
     - Si la descarga falla con `Unauthorized` → `Unavailable(InvalidApiKey)`, aunque haya caché vieja.
     - Si falla con `Network` o `Parse` → caché vieja si existe; si no, `Unavailable(Offline)`.
  3. Sin cliente → caché vieja si existe; si no, `Unavailable(NoApiKey)`.
- Comportamiento de `game_hashes(id)`: primero la caché; si no está, la API (y se guarda); si falla, `None`.

- [ ] **Step 1: Escribir los tests que fallan**

  ```rust
  #[test] fn fresh_cache_skips_network()      // caché t=now-10 + mock que espera 0 llamadas → Available, mock.assert()
  #[test] fn stale_cache_refetches()          // caché vieja + mock 200 → Available con datos nuevos y archivo actualizado
  #[test] fn offline_uses_stale_cache()       // caché vieja + base_url a puerto cerrado → Available(datos viejos)
  #[test] fn offline_without_cache()          // sin caché + puerto cerrado → Unavailable(Offline)
  #[test] fn invalid_key()                    // mock 401 → Unavailable(InvalidApiKey)
  #[test] fn no_key_no_cache()                // client None, sin caché → Unavailable(NoApiKey)
  #[test] fn no_key_with_cache()              // client None, caché vieja → Available
  ```

  Las funciones del llavero no llevan test unitario, porque tocan el llavero real. Se verifican a mano en la Task 7.

- [ ] **Step 2: Ejecutar los tests y confirmar que fallan**

  Run: `cargo test -p ra-hash-checker source`
  Expected: FAIL.

- [ ] **Step 3: Implementar `source.rs`**

  Usar el crate `keyring` (v3, con la feature `windows-native`; en Linux y Mac, las features nativas de cada plataforma).

- [ ] **Step 4: Ejecutar los tests y confirmar que pasan**

  Run: `cargo test -p ra-hash-checker`
  Expected: PASS (toda la suite del backend).

- [ ] **Step 5: Commit**

  ```bash
  git commit -am "feat: fuente de listas con caché, API y llavero"
  ```

---

### Task 6: Comandos Tauri y frontend

**Files:**
- Modify: `src-tauri/src/lib.rs` y `src-tauri/src/main.rs`
- Modify: `src-tauri/tauri.conf.json` (ventana 720×560, título `RA Hash Checker`, `dragDropEnabled: true`)
- Modify: `src-tauri/capabilities/default.json` (permisos `dialog:allow-open`, `opener:allow-open-url`, `clipboard-manager:allow-write-text`, `core:event:default`)
- Create: `index.html`, `src/main.ts`, `src/view.ts`, `src/i18n.ts` y `src/styles.css`
- Test: `src/view.test.ts` y `src/i18n.test.ts` (Vitest)

**Interfaces:**
- Consumes: `expand`, `check` y `check_as` (Task 4); `LiveSource`, `refresh_all`, `load_api_key` y `store_api_key` (Task 5).
- Produces:
  - **Comandos Tauri:**
    - `check_paths(paths: Vec<String>) -> ()`: async. Por cada ruta hace `spawn_blocking`, y emite un evento `"result"` con un `CheckResult` por cada entrada. Si `expand` falla, emite un `CheckResult` con estado `Error`.
    - `check_as(source_path: String, inner_name: Option<String>, console_id: u32) -> CheckResult`
    - `get_settings() -> SettingsView { has_api_key: bool, last_refresh: Option<u64> }`
    - `set_api_key(key: String) -> Result<(), String>`
    - `refresh_lists() -> Result<usize, String>`
  - **Estado:** la caché va en `app.path().app_data_dir()`, con `now` = `SystemTime::now()`.
  - **Frontend:**
    - `view.ts` exporta `renderRow(r: CheckResult, t: Translator): HTMLElement` y `statusLabel(s: Status, t): string`.
    - `i18n.ts` exporta `makeTranslator(lang: string): Translator` y `type Translator = (key: string) => string`.

- [ ] **Step 1: Escribir los tests del frontend que fallan**

  ```ts
  test('statusLabel es', () => { const t = makeTranslator('es-CO'); expect(statusLabel('registered', t)).toBe('✅ Registrado'); expect(statusLabel('registeredNoAchievements', t)).toBe('🟡 Registrado, sin logros todavía'); expect(statusLabel('notRegistered', t)).toBe('❌ No registrado'); expect(statusLabel('unchecked', t)).toBe('⚪ Sin comparar'); })
  test('fallback en', () => { expect(makeTranslator('fr')('status.notRegistered')).toBe('❌ Not registered'); })
  test('row muestra juego y enlace', () => { /* r con game {id: 1234, title:'X'} → el elemento contiene 'X' y un enlace data-url 'https://retroachievements.org/game/1234' */ })
  test('row con discInZip muestra el mensaje', () => { /* message 'discInZip' → texto 'Descomprime el disco y suéltalo de nuevo' */ })
  test('row con alternatives muestra selector', () => { /* alternatives [[9,'Sega CD']] → un <select> con la consola actual + 'Sega CD' */ })
  ```

  Los textos en inglés para `MessageCode` y el resto los decide quien implemente, salvo los estados de arriba.

- [ ] **Step 2: Ejecutar los tests y confirmar que fallan**

  Run: `npx vitest run`
  Expected: FAIL.

- [ ] **Step 3: Implementar el frontend**

  - **Arrastrar y soltar:** `getCurrentWebview().onDragDropEvent`. Al soltar → `invoke('check_paths', { paths })`.
  - **Clic en la zona:** `open({ multiple: true })` de `@tauri-apps/plugin-dialog`.
  - **Filas:** `listen('result', …)` añade una fila por resultado.
  - **Hash:** botón para copiarlo completo (`plugin-clipboard-manager`).
  - **Enlaces:** se abren con `plugin-opener`.
    - Página del juego: `https://retroachievements.org/game/<id>`.
    - Página de claves: `https://retroachievements.org/controlpanel.php`.
  - **Aviso sin clave:** si `get_settings().has_api_key` es falso, la zona de soltar muestra el aviso con el enlace a la página de claves.
  - **Diálogo ⚙:** campo de contraseña para la clave, "Actualizar listas de hashes" y la última actualización formateada con `toLocaleString`.
  - **Selector de consola:** `change` → `invoke('check_as', …)` reemplaza la fila.

- [ ] **Step 4: Implementar los comandos en `lib.rs`**

  Registrar los plugins `dialog`, `opener` y `clipboard-manager`. Un error dentro de un archivo nunca debe cancelar los demás: cada `spawn_blocking` se maneja por separado.

- [ ] **Step 5: Ejecutar los tests y confirmar que pasan**

  Run: `npx vitest run; cargo test --workspace`
  Expected: PASS.

- [ ] **Step 6: Probar en la app**

  Run: `npm run tauri dev`. Soltar un archivo `.gba` cualquiera.
  Expected: aparece la fila con "Game Boy Advance", el hash y "⚪ Sin comparar" más el aviso de clave.

- [ ] **Step 7: Commit**

  ```bash
  git commit -am "feat: interfaz de soltar archivos y comandos Tauri"
  ```

---

### Task 7: Licencia, README, CI y verificación de extremo a extremo

**Files:**
- Create: `LICENSE` (MIT, titular: el usuario, año 2026)
- Create: `README.md` (español e inglés). Debe explicar:
  - qué hace la app;
  - cómo obtener la clave de API;
  - qué consolas se prueban;
  - los no-objetivos;
  - que usa rcheevos, con licencia MIT.
- Create: `.github/workflows/ci.yml`. En `windows-latest`, con `submodules: recursive`: `npm ci`, `npx vitest run` y `cargo test --workspace`.

- [ ] **Step 1: Escribir LICENSE, README y CI**

- [ ] **Step 2: Verificación con datos reales**

  Con `npm run tauri dev`, poner una clave real del usuario. Pedírsela al usuario: que la pegue él en el diálogo, nunca en el chat.

  Expected:
  - El `.cue` de Bloody Roar II muestra PlayStation, `77defab4cedf9f82c4251a474d55cbee` y ✅ con el enlace al juego.
  - Durante el hash de ese `.bin` de 550 MB la ventana sigue respondiendo.
  - Un juego de GBA del usuario muestra su resultado.

- [ ] **Step 3: Verificación de robustez manual**

  Soltar a la vez un `.txt`, una carpeta, un `.zip` con dos ROMs y un `.zip` con un `.cue`.
  Expected: 5 filas (`Unsupported`, `Error`, 2 filas de ROM, `DiscInZip`), sin cierres.

- [ ] **Step 4: Comprobar el instalador**

  Run: `npm run tauri build`
  Expected: se genera el `.msi`/`.exe` en `src-tauri/target/release/bundle/`.

- [ ] **Step 5: Commit**

  ```bash
  git add LICENSE README.md .github/
  git commit -m "docs: licencia MIT, README y CI"
  ```
