//! Fuente real de listas de hashes: caché en disco, API de RA y llavero del sistema.

use crate::cache::HashCache;
use crate::checker::{HashListSource, ListOutcome};
use crate::model::{GameHash, MessageCode};
use crate::ra_api::{RaClient, RaError};
use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};

const KEYRING_SERVICE: &str = "ra-hash-checker";
const KEYRING_USER: &str = "api-key";

/// Serializa "mirar caché -> descargar -> guardar". Se comparte entre todas las `LiveSource`
/// y `refresh_all` (estado administrado de Tauri) para que nunca se descargue la misma lista dos
/// veces a la vez.
pub type FetchLock = Arc<Mutex<()>>;

/// Fallos recordados durante el lote de una `LiveSource`, para no repetir peticiones que ya
/// fallaron (cada una puede tardar hasta el timeout).
#[derive(Default)]
struct Failures {
    unauthorized: bool,
    lists: HashSet<u32>,
    games: HashSet<u32>,
}

pub struct LiveSource {
    cache: HashCache,
    client: Option<RaClient>,
    now: u64,
    lock: FetchLock,
    failures: Mutex<Failures>,
}

/// Recupera el guard aunque otro hilo haya entrado en pánico con él tomado.
fn lock_ignoring_poison<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl LiveSource {
    /// Fuente con su propio cerrojo (tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(cache: HashCache, client: Option<RaClient>, now: u64) -> LiveSource {
        LiveSource::with_lock(cache, client, now, FetchLock::default())
    }

    /// Fuente que comparte `lock` con las demás fuentes y con `refresh_all`.
    pub fn with_lock(cache: HashCache, client: Option<RaClient>, now: u64, lock: FetchLock) -> LiveSource {
        LiveSource { cache, client, now, lock, failures: Mutex::default() }
    }

    fn guard(&self) -> MutexGuard<'_, ()> {
        lock_ignoring_poison(&self.lock)
    }

    fn failures(&self) -> MutexGuard<'_, Failures> {
        lock_ignoring_poison(&self.failures)
    }
}

impl HashListSource for LiveSource {
    fn list(&self, console_id: u32) -> ListOutcome {
        let _guard = self.guard();
        let cached = self.cache.load_list(console_id);
        if let Some(list) = &cached {
            if !HashCache::is_stale(list, self.now) {
                return ListOutcome::Available(list.games.clone());
            }
        }
        let stale_or_offline = |code: MessageCode| match cached.clone() {
            Some(list) => ListOutcome::Available(list.games),
            None => ListOutcome::Unavailable(code),
        };
        let Some(client) = &self.client else {
            return stale_or_offline(MessageCode::NoApiKey);
        };
        {
            let failures = self.failures();
            if failures.unauthorized {
                return ListOutcome::Unavailable(MessageCode::InvalidApiKey);
            }
            if failures.lists.contains(&console_id) {
                return stale_or_offline(MessageCode::Offline);
            }
        }
        match client.get_game_list(console_id) {
            Ok(games) => {
                // Si no se puede guardar, la lista descargada sigue siendo válida.
                let _ = self.cache.save_list(console_id, &games, self.now);
                ListOutcome::Available(games)
            }
            Err(RaError::Unauthorized) => {
                self.failures().unauthorized = true;
                ListOutcome::Unavailable(MessageCode::InvalidApiKey)
            }
            Err(RaError::Network(_)) | Err(RaError::Parse(_)) => {
                self.failures().lists.insert(console_id);
                stale_or_offline(MessageCode::Offline)
            }
        }
    }

    fn game_hashes(&self, game_id: u32) -> Option<Vec<GameHash>> {
        let _guard = self.guard();
        if let Some(h) = self.cache.load_game_hashes(game_id) {
            return Some(h);
        }
        let client = self.client.as_ref()?;
        {
            let failures = self.failures();
            if failures.unauthorized || failures.games.contains(&game_id) {
                return None;
            }
        }
        match client.get_game_hashes(game_id) {
            Ok(hashes) => {
                let _ = self.cache.save_game_hashes(game_id, &hashes);
                Some(hashes)
            }
            Err(e) => {
                let mut failures = self.failures();
                match e {
                    RaError::Unauthorized => failures.unauthorized = true,
                    RaError::Network(_) | RaError::Parse(_) => {
                        failures.games.insert(game_id);
                    }
                }
                None
            }
        }
    }
}

/// Vuelve a descargar todas las listas en caché y devuelve cuántas actualizó.
/// Toma `lock` para cada lista, como `LiveSource`.
pub fn refresh_all(cache: &HashCache, client: &RaClient, now: u64, lock: &Mutex<()>) -> Result<usize, RaError> {
    let mut count = 0;
    for console_id in cache.cached_consoles() {
        let _guard = lock_ignoring_poison(lock);
        let games = client.get_game_list(console_id)?;
        cache
            .save_list(console_id, &games, now)
            .map_err(|e| RaError::Network(e.to_string()))?;
        count += 1;
    }
    Ok(count)
}

fn entry() -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
}

/// `None` si no hay clave guardada (o el llavero no está disponible).
pub fn load_api_key() -> Option<String> {
    entry().ok()?.get_password().ok().filter(|k| !k.is_empty())
}

/// Guarda la clave en el llavero del sistema; una clave vacía borra la entrada.
pub fn store_api_key(key: &str) -> keyring::Result<()> {
    let entry = entry()?;
    if key.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e),
        }
    } else {
        entry.set_password(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GameEntry;
    use std::sync::Arc;

    const NOW: u64 = 10_000_000;
    const STALE: u64 = NOW - 8 * 24 * 3600;
    const DEAD: &str = "http://127.0.0.1:1";

    fn game(id: u32, title: &str) -> GameEntry {
        GameEntry { id, title: title.into(), console_id: 5, num_achievements: 1, hashes: vec!["AA".into()] }
    }

    fn cache_with(dir: &tempfile::TempDir, fetched_at: u64, title: &str) -> HashCache {
        let c = HashCache::new(dir.path().to_path_buf());
        c.save_list(5, &[game(1, title)], fetched_at).unwrap();
        c
    }

    fn body(title: &str) -> String {
        format!(r#"[{{"ID":2,"Title":"{title}","ConsoleID":5,"NumAchievements":3,"Hashes":["BB"]}}]"#)
    }

    fn titles(o: ListOutcome) -> Vec<String> {
        match o {
            ListOutcome::Available(g) => g.into_iter().map(|g| g.title).collect(),
            ListOutcome::Unavailable(c) => panic!("unavailable: {c:?}"),
        }
    }

    #[test]
    fn live_source_is_send_sync() {
        fn check<T: Send + Sync>() {}
        check::<LiveSource>();
    }

    #[test]
    fn fresh_cache_skips_network() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache_with(&dir, NOW - 10, "Old");
        let mut server = mockito::Server::new();
        let m = server.mock("GET", mockito::Matcher::Any).expect(0).create();
        let src = LiveSource::new(cache, Some(RaClient::new(&server.url(), "K".into())), NOW);
        assert_eq!(titles(src.list(5)), vec!["Old"]);
        m.assert();
    }

    #[test]
    fn stale_cache_refetches() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache_with(&dir, STALE, "Old");
        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/API_GetGameList.php")
            .match_query(mockito::Matcher::Any)
            .with_body(body("New"))
            .expect(1)
            .create();
        let src = LiveSource::new(cache, Some(RaClient::new(&server.url(), "K".into())), NOW);
        assert_eq!(titles(src.list(5)), vec!["New"]);
        m.assert();
        let saved = HashCache::new(dir.path().to_path_buf()).load_list(5).unwrap();
        assert_eq!(saved.fetched_at, NOW);
        assert_eq!(saved.games[0].title, "New");
    }

    #[test]
    fn offline_uses_stale_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache_with(&dir, STALE, "Old");
        let src = LiveSource::new(cache, Some(RaClient::new(DEAD, "K".into())), NOW);
        assert_eq!(titles(src.list(5)), vec!["Old"]);
    }

    #[test]
    fn offline_without_cache() {
        let dir = tempfile::tempdir().unwrap();
        let src = LiveSource::new(HashCache::new(dir.path().to_path_buf()), Some(RaClient::new(DEAD, "K".into())), NOW);
        assert!(matches!(src.list(5), ListOutcome::Unavailable(MessageCode::Offline)));
    }

    #[test]
    fn invalid_key() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache_with(&dir, STALE, "Old");
        let mut server = mockito::Server::new();
        let _m = server.mock("GET", mockito::Matcher::Any).with_status(401).create();
        let src = LiveSource::new(cache, Some(RaClient::new(&server.url(), "K".into())), NOW);
        assert!(matches!(src.list(5), ListOutcome::Unavailable(MessageCode::InvalidApiKey)));
    }

    #[test]
    fn no_key_no_cache() {
        let dir = tempfile::tempdir().unwrap();
        let src = LiveSource::new(HashCache::new(dir.path().to_path_buf()), None, NOW);
        assert!(matches!(src.list(5), ListOutcome::Unavailable(MessageCode::NoApiKey)));
    }

    #[test]
    fn no_key_with_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache_with(&dir, STALE, "Old");
        let src = LiveSource::new(cache, None, NOW);
        assert_eq!(titles(src.list(5)), vec!["Old"]);
    }

    #[test]
    fn concurrent_lists_download_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/API_GetGameList.php")
            .match_query(mockito::Matcher::Any)
            .with_body(body("New"))
            .expect(1)
            .create();
        let src = Arc::new(LiveSource::new(
            HashCache::new(dir.path().to_path_buf()),
            Some(RaClient::new(&server.url(), "K".into())),
            NOW,
        ));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let s = Arc::clone(&src);
                std::thread::spawn(move || titles(s.list(5)))
            })
            .collect();
        for h in handles {
            assert_eq!(h.join().unwrap(), vec!["New"]);
        }
        m.assert();
    }

    #[test]
    fn game_hashes_cache_then_api() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/API_GetGameHashes.php")
            .match_query(mockito::Matcher::Any)
            .with_body(r#"{"Results":[{"MD5":"AA","Name":"n","Labels":["nes"]}]}"#)
            .expect(1)
            .create();
        let src = LiveSource::new(
            HashCache::new(dir.path().to_path_buf()),
            Some(RaClient::new(&server.url(), "K".into())),
            NOW,
        );
        assert_eq!(src.game_hashes(7).unwrap().len(), 1);
        assert_eq!(src.game_hashes(7).unwrap().len(), 1);
        m.assert();
    }

    #[test]
    fn refresh_all_redownloads_cached_consoles() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache_with(&dir, NOW - 100, "Old");
        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/API_GetGameList.php")
            .match_query(mockito::Matcher::Any)
            .with_body(body("New"))
            .expect(1)
            .create();
        let client = RaClient::new(&server.url(), "K".into());
        assert_eq!(refresh_all(&cache, &client, NOW, &Mutex::new(())).unwrap(), 1);
        m.assert();
        assert_eq!(cache.load_list(5).unwrap().games[0].title, "New");
    }

    fn slow_list_mock(server: &mut mockito::ServerGuard) -> mockito::Mock {
        server
            .mock("GET", "/API_GetGameList.php")
            .match_query(mockito::Matcher::Any)
            .with_chunked_body(|w| {
                std::thread::sleep(std::time::Duration::from_millis(300));
                w.write_all(body("New").as_bytes())
            })
            .expect(1)
            .create()
    }

    #[test]
    fn sources_sharing_a_lock_download_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let m = slow_list_mock(&mut server);
        let lock: FetchLock = Arc::default();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let src = LiveSource::with_lock(
                    HashCache::new(dir.path().to_path_buf()),
                    Some(RaClient::new(&server.url(), "K".into())),
                    NOW,
                    Arc::clone(&lock),
                );
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    titles(src.list(5))
                })
            })
            .collect();
        for h in handles {
            assert_eq!(h.join().unwrap(), vec!["New"]);
        }
        m.assert();
    }

    #[test]
    fn refresh_all_waits_for_the_fetch_lock() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache_with(&dir, NOW - 100, "Old");
        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/API_GetGameList.php")
            .match_query(mockito::Matcher::Any)
            .with_body(body("New"))
            .expect(1)
            .create();
        let lock: FetchLock = Arc::default();
        let held = lock.lock().unwrap();
        let url = server.url();
        let lock2 = Arc::clone(&lock);
        let dir_path = dir.path().to_path_buf();
        let t = std::thread::spawn(move || {
            let client = RaClient::new(&url, "K".into());
            refresh_all(&HashCache::new(dir_path), &client, NOW, &lock2).unwrap()
        });
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(!m.matched(), "refresh_all fetched while the lock was held");
        drop(held);
        assert_eq!(t.join().unwrap(), 1);
        m.assert();
        assert_eq!(cache.load_list(5).unwrap().games[0].title, "New");
    }

    #[test]
    fn network_failure_is_remembered_per_console() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let m = server.mock("GET", mockito::Matcher::Any).with_status(500).expect(1).create();
        let src = LiveSource::new(
            HashCache::new(dir.path().to_path_buf()),
            Some(RaClient::new(&server.url(), "K".into())),
            NOW,
        );
        assert!(matches!(src.list(5), ListOutcome::Unavailable(MessageCode::Offline)));
        assert!(matches!(src.list(5), ListOutcome::Unavailable(MessageCode::Offline)));
        m.assert();
    }

    #[test]
    fn remembered_failure_still_uses_stale_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache_with(&dir, STALE, "Old");
        let mut server = mockito::Server::new();
        let m = server.mock("GET", mockito::Matcher::Any).with_status(503).expect(1).create();
        let src = LiveSource::new(cache, Some(RaClient::new(&server.url(), "K".into())), NOW);
        assert_eq!(titles(src.list(5)), vec!["Old"]);
        assert_eq!(titles(src.list(5)), vec!["Old"]);
        m.assert();
    }

    #[test]
    fn unauthorized_is_remembered_for_every_request() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let m = server.mock("GET", mockito::Matcher::Any).with_status(401).expect(1).create();
        let src = LiveSource::new(
            HashCache::new(dir.path().to_path_buf()),
            Some(RaClient::new(&server.url(), "K".into())),
            NOW,
        );
        assert!(matches!(src.list(5), ListOutcome::Unavailable(MessageCode::InvalidApiKey)));
        assert!(matches!(src.list(7), ListOutcome::Unavailable(MessageCode::InvalidApiKey)));
        assert!(src.game_hashes(3).is_none());
        m.assert();
    }

    #[test]
    fn game_hashes_failure_is_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = mockito::Server::new();
        let m = server.mock("GET", mockito::Matcher::Any).with_status(500).expect(1).create();
        let src = LiveSource::new(
            HashCache::new(dir.path().to_path_buf()),
            Some(RaClient::new(&server.url(), "K".into())),
            NOW,
        );
        assert!(src.game_hashes(7).is_none());
        assert!(src.game_hashes(7).is_none());
        m.assert();
    }
}
