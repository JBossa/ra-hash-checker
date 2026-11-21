//! Caché en disco de las listas de hashes de RetroAchievements (una por consola).

use crate::model::{GameEntry, GameHash};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

/// `<ruta>.<pid>.<n>.tmp`: único por proceso y por escritura, para que dos escrituras simultáneas
/// de la misma lista nunca compartan el archivo temporal.
fn tmp_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.{n}.tmp", std::process::id()));
    PathBuf::from(tmp)
}

/// Escribe en un temporal único y renombra, para que un cierre a medias no corrompa la caché.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path(path);
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
    fn tmp_names_are_unique_per_write() {
        let p = Path::new("lists").join("5.json");
        let a = tmp_path(&p);
        let b = tmp_path(&p);
        assert_ne!(a, b);
        let name = a.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("5.json."), "{name}");
        assert!(name.contains(&std::process::id().to_string()), "{name}");
        assert!(name.ends_with(".tmp"), "{name}");
        assert_eq!(a.parent(), p.parent());
    }

    #[test]
    fn concurrent_saves_leave_a_valid_list() {
        let (t, c) = cache();
        let c = std::sync::Arc::new(c);
        let handles: Vec<_> = (0..8u32)
            .map(|i| {
                let c = std::sync::Arc::clone(&c);
                std::thread::spawn(move || {
                    let games: Vec<GameEntry> = (0..500).map(|g| game(g * 10 + i, &["aa"])).collect();
                    for _ in 0..5 {
                        let _ = c.save_list(5, &games, 100 + u64::from(i));
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(c.load_list(5).unwrap().games.len(), 500);
        let leftovers: Vec<_> = std::fs::read_dir(t.path().join("lists"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
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
