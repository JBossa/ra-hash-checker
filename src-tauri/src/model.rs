//! Tipos compartidos entre la API de RA, el motor de comprobación y el frontend.

use serde::{Deserialize, Serialize};

/// Entrada de `API_GetGameList.php` (campos PascalCase en el JSON de RA).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct GameEntry {
    #[serde(rename = "ID")]
    pub id: u32,
    pub title: String,
    #[serde(rename = "ConsoleID")]
    pub console_id: u32,
    pub num_achievements: u32,
    #[serde(default)]
    pub hashes: Vec<String>,
}

/// Entrada de `Results` en `API_GetGameHashes.php`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct GameHash {
    #[serde(rename = "MD5")]
    pub md5: String,
    pub name: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Registered,
    RegisteredNoAchievements,
    NotRegistered,
    Unchecked,
    Unsupported,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MessageCode {
    DiscInZip,
    NoHash,
    ReadError,
    InvalidApiKey,
    Offline,
    NoApiKey,
    ArchiveNotSupported,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub file_name: String,
    pub source_path: String,
    pub inner_name: Option<String>,
    pub console_id: Option<u32>,
    pub console_name: Option<String>,
    pub md5: Option<String>,
    pub status: Status,
    pub game: Option<GameEntry>,
    pub matched_name: Option<String>,
    /// `(console_id, console_name)` para el selector de consola.
    pub alternatives: Vec<(u32, String)>,
    pub message: Option<MessageCode>,
}
