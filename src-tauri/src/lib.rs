mod cache;
mod checker;
mod inputs;
mod model;
mod ra_api;
mod source;
mod tools;

use std::path::{Path, PathBuf};
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::cache::HashCache;
use crate::model::{CheckResult, GameInfo, MessageCode, Status};
use crate::ra_api::{RaClient, RaError};
use crate::source::{FetchLock, LiveSource};
use crate::tools::{ToolError, ToolSpec, ToolStatus, ToolsConfig};

const RA_BASE_URL: &str = "https://retroachievements.org/API";

/// Cerrojo de descargas compartido por todos los comandos (estado administrado de Tauri), para que
/// dos lotes, `check_as` y `refresh_lists` nunca descarguen la misma lista a la vez.
#[derive(Default)]
struct SharedFetchLock(FetchLock);

/// Una sola conversión con herramientas externas a la vez en toda la app (cada una puede escribir
/// varios GB).
#[derive(Default)]
struct ConvertLock(Arc<Mutex<()>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    has_api_key: bool,
    last_refresh: Option<u64>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("cache");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn tools_file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("tools.json"))
}

/// Carpeta de las imágenes temporales de las herramientas externas.
fn tools_tmp(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_cache_dir().map_err(|e| e.to_string())?.join("tmp"))
}

/// Lo necesario para crear una `LiveSource`, sin crear todavía el cliente HTTP.
struct SourceParts {
    dir: PathBuf,
    key: Option<String>,
    lock: FetchLock,
}

impl SourceParts {
    fn from_app(app: &AppHandle) -> Result<SourceParts, String> {
        Ok(SourceParts {
            dir: cache_dir(app)?,
            key: source::load_api_key(),
            lock: Arc::clone(&app.state::<SharedFetchLock>().0),
        })
    }

    fn build(self) -> LiveSource {
        let client = self.key.map(|k| RaClient::new(RA_BASE_URL, k));
        LiveSource::with_lock(HashCache::new(self.dir), client, now_secs(), self.lock)
    }
}

/// Ejecuta `f` con una `LiveSource` en un hilo bloqueante. La fuente (y su cliente HTTP bloqueante
/// de reqwest) se crea y se destruye dentro de ese hilo: hacerlo en el runtime async hace caer la app.
async fn with_source<R: Send + 'static>(
    parts: SourceParts,
    f: impl FnOnce(&LiveSource) -> R + Send + 'static,
) -> Result<R, String> {
    tauri::async_runtime::spawn_blocking(move || f(&parts.build()))
        .await
        .map_err(|e| e.to_string())
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn error_row(path: &str, code: MessageCode) -> CheckResult {
    CheckResult {
        file_name: file_name_of(Path::new(path)),
        source_path: path.to_string(),
        inner_name: None,
        console_id: None,
        console_name: None,
        md5: None,
        status: Status::Error,
        game: None,
        matched_name: None,
        game_hashes: Vec::new(),
        alternatives: Vec::new(),
        message: Some(code),
        tool: None,
        detail: None,
    }
}

/// Resultados de una ruta: siempre al menos una fila.
fn check_one(path: &str, src: &LiveSource) -> Vec<CheckResult> {
    match inputs::expand(Path::new(path)) {
        Err(code) => vec![error_row(path, code)],
        Ok(list) if list.is_empty() => vec![error_row(path, MessageCode::ReadError)],
        Ok(list) => list.iter().map(|i| checker::check(i, src)).collect(),
    }
}

/// Evento `progress`: un archivo espera o está en conversión con una herramienta externa.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    source_path: String,
    file_name: String,
    tool: &'static str,
    /// `"queued"` o `"converting"`.
    stage: &'static str,
}

/// Archivo que se convertirá con una herramienta conectada.
struct ToolJob {
    path: String,
    spec: &'static ToolSpec,
    exe: PathBuf,
}

fn tool_row(path: &str, spec: &ToolSpec, code: MessageCode, detail: Option<String>) -> CheckResult {
    CheckResult {
        status: Status::Error,
        tool: Some(spec.short_name.to_string()),
        detail,
        ..error_row(path, code)
    }
}

/// Convierte con la herramienta, comprueba la imagen solo con sus consolas y la borra.
fn check_with_tool(job: &ToolJob, tmp: &Path, src: &LiveSource) -> CheckResult {
    match tools::convert(job.spec, &job.exe, Path::new(&job.path), tmp) {
        Err(ToolError::NoSpace(bytes)) => {
            let gb = format!("{:.1}", bytes as f64 / 1e9);
            tool_row(&job.path, job.spec, MessageCode::NoDiskSpace, Some(gb))
        }
        Err(ToolError::Failed) => tool_row(&job.path, job.spec, MessageCode::ToolFailed, None),
        Ok(image) => {
            let input = inputs::Input {
                source_path: image.path.clone(),
                display_name: file_name_of(Path::new(&job.path)),
                inner_name: None,
                data: None,
                disc_in_zip: false,
            };
            let mut r = checker::check_only(&input, job.spec.consoles, src);
            r.source_path = job.path.clone();
            r
        } // `image` se borra aquí
    }
}

/// Separa los archivos que se convertirán con una herramienta conectada; el resto se comprueba
/// como siempre (sin herramienta conectada, el comprobador pide conectarla).
fn split_tool_jobs(paths: Vec<String>, config: &ToolsConfig) -> (Vec<ToolJob>, Vec<String>) {
    let mut jobs = Vec::new();
    let mut rest = Vec::new();
    for path in paths {
        let tool = tools::tool_for_path(Path::new(&path))
            .and_then(|spec| config.usable(spec.id).map(|exe| (spec, exe.to_path_buf())));
        match tool {
            Some((spec, exe)) => jobs.push(ToolJob { path, spec, exe }),
            None => rest.push(path),
        }
    }
    (jobs, rest)
}

/// Archivos comprobados a la vez: cada uno puede tener hasta 64 MB en memoria.
const MAX_PARALLEL_FILES: usize = 4;

/// Ejecuta `f` para cada elemento, en orden de llegada, con a lo sumo `limit` hilos a la vez.
/// Un pánico en un elemento solo pierde ese elemento.
fn for_each_bounded<T: Send>(items: Vec<T>, limit: usize, f: impl Fn(T) + Sync) {
    let workers = limit.max(1).min(items.len());
    let queue = Mutex::new(items.into_iter());
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let next = queue.lock().unwrap_or_else(|e| e.into_inner()).next();
                let Some(item) = next else { break };
                let _ = std::panic::catch_unwind(AssertUnwindSafe(|| f(item)));
            });
        }
    });
}

#[tauri::command]
async fn check_paths(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    let parts = SourceParts::from_app(&app)?;
    let (jobs, rest) = split_tool_jobs(paths, &ToolsConfig::load(&tools_file(&app)?));
    let tmp = tools_tmp(&app)?;
    let convert_lock = Arc::clone(&app.state::<ConvertLock>().0);
    // Fuera del hilo de la UI; cada fila se emite en cuanto termina su archivo. Las conversiones
    // van en su propia cola, de una en una, para no frenar los archivos normales.
    with_source(parts, move |src| {
        let progress = |job: &ToolJob, stage| {
            let _ = app.emit(
                "progress",
                Progress {
                    source_path: job.path.clone(),
                    file_name: file_name_of(Path::new(&job.path)),
                    tool: job.spec.short_name,
                    stage,
                },
            );
        };
        for job in &jobs {
            progress(job, "queued");
        }
        std::thread::scope(|s| {
            s.spawn(|| {
                for job in &jobs {
                    let _guard = convert_lock.lock().unwrap_or_else(|e| e.into_inner());
                    progress(job, "converting");
                    let r = std::panic::catch_unwind(AssertUnwindSafe(|| check_with_tool(job, &tmp, src)))
                        .unwrap_or_else(|_| tool_row(&job.path, job.spec, MessageCode::ToolFailed, None));
                    let _ = app.emit("result", r);
                }
            });
            for_each_bounded(rest, MAX_PARALLEL_FILES, |path| {
                for r in check_one(&path, src) {
                    let _ = app.emit("result", r);
                }
            });
        });
    })
    .await
}

#[tauri::command]
async fn check_as(
    app: AppHandle,
    source_path: String,
    inner_name: Option<String>,
    console_id: u32,
) -> Result<CheckResult, String> {
    let parts = SourceParts::from_app(&app)?;
    with_source(parts, move |src| match inputs::expand(Path::new(&source_path)) {
        Err(code) => error_row(&source_path, code),
        Ok(list) => match list.iter().find(|i| i.inner_name == inner_name) {
            Some(input) => checker::check_as(input, console_id, src),
            None => error_row(&source_path, MessageCode::ReadError),
        },
    })
    .await
}

#[tauri::command]
async fn game_info(app: AppHandle, game_id: u32) -> Result<Option<GameInfo>, String> {
    let parts = SourceParts::from_app(&app)?;
    with_source(parts, move |src| src.game_info(game_id)).await
}

/// Consola y extensiones que la app sabe comprobar.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsoleFormats {
    console_id: u32,
    console_name: &'static str,
    extensions: Vec<&'static str>,
}

#[tauri::command]
fn supported_formats() -> Vec<ConsoleFormats> {
    rcheevos_hash::formats_by_console()
        .into_iter()
        .map(|(console_id, extensions)| ConsoleFormats {
            console_id,
            console_name: rcheevos_hash::console_name(console_id),
            extensions,
        })
        .collect()
}

/// Estado de una herramienta externa para Ajustes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolView {
    id: &'static str,
    name: &'static str,
    short_name: &'static str,
    path: Option<String>,
    status: ToolStatus,
    extensions: &'static [&'static str],
    consoles: Vec<&'static str>,
    download_url: &'static str,
}

fn tool_view(spec: &'static ToolSpec, config: &ToolsConfig) -> ToolView {
    ToolView {
        id: spec.id,
        name: spec.name,
        short_name: spec.short_name,
        path: config.path_for(spec.id).map(|p| p.to_string_lossy().into_owned()),
        status: config.status(spec.id),
        extensions: spec.extensions,
        consoles: spec.consoles.iter().map(|&c| rcheevos_hash::console_name(c)).collect(),
        download_url: spec.download_url,
    }
}

fn tool_spec(id: &str) -> Result<&'static ToolSpec, String> {
    tools::tool_by_id(id).ok_or_else(|| "unknownTool".to_string())
}

#[tauri::command]
fn get_tools(app: AppHandle) -> Result<Vec<ToolView>, String> {
    let config = ToolsConfig::load(&tools_file(&app)?);
    Ok(tools::TOOLS.iter().map(|&spec| tool_view(spec, &config)).collect())
}

/// Busca la herramienta en el equipo; si la encuentra (y es válida), la guarda.
#[tauri::command]
async fn detect_tool(app: AppHandle, id: String) -> Result<ToolView, String> {
    let spec = tool_spec(&id)?;
    let file = tools_file(&app)?;
    let found = tauri::async_runtime::spawn_blocking(move || tools::detect(spec))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "toolNotFound".to_string())?;
    let mut config = ToolsConfig::load(&file);
    config.set(spec.id, found);
    config.save(&file).map_err(|_| "toolSaveFailed".to_string())?;
    Ok(tool_view(spec, &config))
}

/// Guarda la ruta elegida por el usuario si el ejecutable es la herramienta.
#[tauri::command]
async fn set_tool_path(app: AppHandle, id: String, path: String) -> Result<ToolView, String> {
    let spec = tool_spec(&id)?;
    let file = tools_file(&app)?;
    let exe = PathBuf::from(&path);
    let check = exe.clone();
    let valid = tauri::async_runtime::spawn_blocking(move || tools::validate(spec, &check))
        .await
        .map_err(|e| e.to_string())?;
    if !valid {
        return Err("invalidTool".to_string());
    }
    let mut config = ToolsConfig::load(&file);
    config.set(spec.id, exe);
    config.save(&file).map_err(|_| "toolSaveFailed".to_string())?;
    Ok(tool_view(spec, &config))
}

#[tauri::command]
fn clear_tool(app: AppHandle, id: String) -> Result<ToolView, String> {
    let spec = tool_spec(&id)?;
    let file = tools_file(&app)?;
    let mut config = ToolsConfig::load(&file);
    config.clear(spec.id);
    config.save(&file).map_err(|_| "toolSaveFailed".to_string())?;
    Ok(tool_view(spec, &config))
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Result<SettingsView, String> {
    Ok(SettingsView {
        has_api_key: source::load_api_key().is_some(),
        last_refresh: HashCache::new(cache_dir(&app)?).last_refresh(),
    })
}

#[tauri::command]
fn set_api_key(key: String) -> Result<(), String> {
    source::store_api_key(key.trim()).map_err(|_| "keyStoreFailed".to_string())
}

#[tauri::command]
async fn refresh_lists(app: AppHandle) -> Result<usize, String> {
    let key = source::load_api_key().ok_or_else(|| "noApiKey".to_string())?;
    let dir = cache_dir(&app)?;
    let lock = Arc::clone(&app.state::<SharedFetchLock>().0);
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let client = RaClient::new(RA_BASE_URL, key);
        source::refresh_all(&HashCache::new(dir), &client, now_secs(), &lock)
    })
    .await
    .map_err(|_| "offline".to_string())?;
    joined.map_err(|e| {
        match e {
            RaError::Unauthorized => "invalidApiKey",
            _ => "offline",
        }
        .to_string()
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SharedFetchLock::default())
        .manage(ConvertLock::default())
        .setup(|app| {
            // Restos de una conversión interrumpida al cerrar la app.
            if let Ok(tmp) = tools_tmp(app.handle()) {
                tools::clean_tmp(&tmp);
            }
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            check_paths,
            check_as,
            game_info,
            supported_formats,
            get_tools,
            detect_tool,
            set_tool_path,
            clear_tool,
            get_settings,
            set_api_key,
            refresh_lists
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn bounded_runs_every_item_at_most_four_at_a_time() {
        let running = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let done = Mutex::new(Vec::new());
        for_each_bounded((0..20).collect(), MAX_PARALLEL_FILES, |i: u32| {
            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(30));
            running.fetch_sub(1, Ordering::SeqCst);
            done.lock().unwrap().push(i);
        });
        let mut done = done.into_inner().unwrap();
        done.sort_unstable();
        assert_eq!(done, (0..20).collect::<Vec<_>>());
        let peak = peak.into_inner();
        assert!(peak <= 4, "peak {peak}");
        assert!(peak >= 2, "no parallelism: peak {peak}");
    }

    /// Regresión: con una clave guardada, el cliente HTTP bloqueante de reqwest no puede crearse
    /// (ni destruirse) dentro del runtime async; en debug eso hacía caer cada comprobación.
    #[test]
    fn source_with_key_works_from_an_async_command() {
        let dir = tempfile::tempdir().unwrap();
        let parts = SourceParts {
            dir: dir.path().to_path_buf(),
            key: Some("KEY".into()),
            lock: FetchLock::default(),
        };
        let r = tauri::async_runtime::block_on(async move {
            tauri::async_runtime::spawn(async move { with_source(parts, |_| 42).await })
                .await
                .unwrap()
        });
        assert_eq!(r, Ok(42));
    }

    #[test]
    fn only_connected_tool_formats_are_queued_for_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("DolphinTool.exe");
        std::fs::write(&exe, b"x").unwrap();
        let paths = vec!["a.gba".to_string(), "b.RVZ".to_string(), "c.wia".to_string(), "d.iso".to_string()];
        let (jobs, rest) = split_tool_jobs(paths.clone(), &ToolsConfig::default());
        assert!(jobs.is_empty());
        assert_eq!(rest, paths);
        let mut config = ToolsConfig::default();
        config.set("dolphin", exe);
        let (jobs, rest) = split_tool_jobs(paths, &config);
        assert_eq!(jobs.iter().map(|j| j.path.as_str()).collect::<Vec<_>>(), ["b.RVZ", "c.wia"]);
        assert_eq!(rest, ["a.gba", "d.iso"]);
    }

    #[cfg(windows)]
    #[test]
    fn failed_tool_conversion_becomes_a_tool_failed_row() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("game.rvz");
        std::fs::write(&input, b"rvz").unwrap();
        let fail = dir.path().join("fail.bat");
        std::fs::write(&fail, b"@exit /b 1\r\n").unwrap();
        let job = ToolJob { path: input.to_string_lossy().into_owned(), spec: &tools::DOLPHIN, exe: fail };
        let src = LiveSource::new(HashCache::new(dir.path().join("cache")), None, 0);
        let r = check_with_tool(&job, &dir.path().join("tmp"), &src);
        assert_eq!(r.message, Some(MessageCode::ToolFailed));
        assert_eq!(r.tool.as_deref(), Some("Dolphin"));
        assert_eq!(r.file_name, "game.rvz");
    }

    #[test]
    fn bounded_survives_a_panicking_item() {
        let done = AtomicUsize::new(0);
        for_each_bounded((0..10).collect(), 4, |i: u32| {
            if i == 3 {
                panic!("boom");
            }
            done.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(done.into_inner(), 9);
    }
}
