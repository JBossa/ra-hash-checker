//! Caché en disco de las listas de hashes de RetroAchievements (una por consola).

use crate::model::{GameEntry, GameHash};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Una lista se considera obsoleta pasados 7 días.
const MAX_AGE_SECS: u64 = 7 * 24 * 3600;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CachedList {
    /// Segundos Unix en que se descargó la lista.
    pub fetched_at: u64,
    pub games: Vec<GameEntry>,
}

pub struct HashCache {
    dir: PathBuf,
}

impl HashCache {
    pub fn new(dir: PathBuf) -> HashCache {
        HashCache { dir }
    }

    fn list_path(&self, console_id: u32) -> PathBuf {
        self.dir.join("lists").join(format!("{console_id}.json"))
    }

    fn game_path(&self, game_id: u32) -> PathBuf {
        self.dir.join("games").join(format!("{game_id}.json"))
    }

    /// Devuelve `None` si el archivo no existe o está corrupto.
    pub fn load_list(&self, console_id: u32) -> Option<CachedList> {
        let bytes = std::fs::read(self.list_path(console_id)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn save_list(&self, console_id: u32, games: &[GameEntry], now: u64) -> std::io::Result<()> {
        let list = CachedList { fetched_at: now, games: games.to_vec() };
        write_atomic(&self.list_path(console_id), &serde_json::to_vec(&list)?)
    }

    pub fn is_stale(list: &CachedList, now: u64) -> bool {
        now.saturating_sub(list.fetched_at) > MAX_AGE_SECS
    }

    /// Busca por MD5 sin distinguir mayúsculas (solo lo usan los tests; checker.rs tiene su propia búsqueda).
    #[allow(dead_code)]
    pub fn find<'a>(list: &'a CachedList, md5: &str) -> Option<&'a GameEntry> {
        list.games
            .iter()
            .find(|g| g.hashes.iter().any(|h| h.eq_ignore_ascii_case(md5)))
    }

    pub fn load_game_hashes(&self, game_id: u32) -> Option<Vec<GameHash>> {
        let bytes = std::fs::read(self.game_path(game_id)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn save_game_hashes(&self, game_id: u32, hashes: &[GameHash]) -> std::io::Result<()> {
        write_atomic(&self.game_path(game_id), &serde_json::to_vec(hashes)?)
    }

    /// Consolas con una lista en caché que se puede leer.
    pub fn cached_consoles(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = std::fs::read_dir(self.dir.join("lists"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                if path.extension()? != "json" {
                    return None;
                }
                let id: u32 = path.file_stem()?.to_str()?.parse().ok()?;
                self.load_list(id).map(|_| id)
            })
            .collect();
        ids.sort_unstable();
        ids
    }

    /// El `fetched_at` más reciente de todas las listas.
    pub fn last_refresh(&self) -> Option<u64> {
        self.cached_consoles()
            .into_iter()
            .filter_map(|id| self.load_list(id))
            .map(|l| l.fetched_at)
            .max()
    }
}

/// Escribe en `<ruta>.tmp` y renombra, para que un cierre a medias no corrompa la caché.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(id: u32, hashes: &[&str]) -> GameEntry {
        GameEntry {
            id,
            title: format!("Game {id}"),
            console_id: 5,
            num_achievements: 10,
            hashes: hashes.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn cache() -> (tempfile::TempDir, HashCache) {
        let t = tempfile::tempdir().unwrap();
        let c = HashCache::new(t.path().to_path_buf());
        (t, c)
    }

    #[test]
    fn roundtrip_list() {
        let (_t, c) = cache();
        let games = vec![game(1, &["aa"]), game(2, &["bb"])];
        c.save_list(5, &games, 1000).unwrap();
        let l = c.load_list(5).unwrap();
        assert_eq!(l.fetched_at, 1000);
        assert_eq!(l.games, games);
    }

    #[test]
    fn missing_list_is_none() {
        let (_t, c) = cache();
        assert!(c.load_list(7).is_none());
    }

    #[test]
    fn corrupt_file_is_none() {
        let (t, c) = cache();
        std::fs::create_dir_all(t.path().join("lists")).unwrap();
        std::fs::write(t.path().join("lists/5.json"), "{").unwrap();
        assert!(c.load_list(5).is_none());
    }

    #[test]
    fn staleness_boundary() {
        let l = CachedList { fetched_at: 0, games: vec![] };
        assert!(!HashCache::is_stale(&l, 604800));
        assert!(HashCache::is_stale(&l, 604801));
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let l = CachedList {
            fetched_at: 0,
            games: vec![game(1, &["0123"]), game(2, &["ABCDEF0123456789ABCDEF0123456789"])],
        };
        let g = HashCache::find(&l, "abcdef0123456789abcdef0123456789").unwrap();
        assert_eq!(g.id, 2);
        assert!(HashCache::find(&l, "ffff").is_none());
    }

    #[test]
    fn cached_consoles_and_last_refresh() {
        let (_t, c) = cache();
        assert_eq!(c.last_refresh(), None);
        c.save_list(5, &[game(1, &[])], 10).unwrap();
        c.save_list(12, &[game(2, &[])], 20).unwrap();
        let cons = c.cached_consoles();
        assert!(cons.contains(&5) && cons.contains(&12));
        assert_eq!(c.last_refresh(), Some(20));
    }
}
