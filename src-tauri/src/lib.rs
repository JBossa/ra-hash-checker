mod cache;
mod checker;
mod inputs;
mod model;
mod ra_api;
mod source;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::cache::HashCache;
use crate::model::{CheckResult, MessageCode, Status};
use crate::ra_api::{RaClient, RaError};
use crate::source::LiveSource;

const RA_BASE_URL: &str = "https://retroachievements.org/API";

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

fn new_source(app: &AppHandle) -> Result<LiveSource, String> {
    let client = source::load_api_key().map(|k| RaClient::new(RA_BASE_URL, k));
    Ok(LiveSource::new(
        HashCache::new(cache_dir(app)?),
        client,
        now_secs(),
    ))
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
        alternatives: Vec::new(),
        message: Some(code),
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

#[tauri::command]
async fn check_paths(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    let src = Arc::new(new_source(&app)?);
    let mut handles = Vec::new();
    for path in paths {
        let src = Arc::clone(&src);
        let app = app.clone();
        let handle = tauri::async_runtime::spawn_blocking(move || {
            for r in check_one(&path, &src) {
                let _ = app.emit("result", r);
            }
        });
        handles.push(handle);
    }
    for h in handles {
        // Un fallo en una ruta no afecta a las demás.
        let _ = h.await;
    }
    Ok(())
}

#[tauri::command]
async fn check_as(
    app: AppHandle,
    source_path: String,
    inner_name: Option<String>,
    console_id: u32,
) -> Result<CheckResult, String> {
    let src = new_source(&app)?;
    let joined = tauri::async_runtime::spawn_blocking(move || {
        match inputs::expand(Path::new(&source_path)) {
            Err(code) => error_row(&source_path, code),
            Ok(list) => match list.iter().find(|i| i.inner_name == inner_name) {
                Some(input) => checker::check_as(input, console_id, &src),
                None => error_row(&source_path, MessageCode::ReadError),
            },
        }
    })
    .await;
    Ok(joined.map_err(|e| e.to_string())?)
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
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let client = RaClient::new(RA_BASE_URL, key);
        source::refresh_all(&HashCache::new(dir), &client, now_secs())
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
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            check_paths,
            check_as,
            get_settings,
            set_api_key,
            refresh_lists
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
