//! Comprobación de un archivo contra las listas de hashes registradas en RA.

use crate::inputs::Input;
use crate::model::{CheckResult, GameEntry, GameHash, MessageCode, Status};
use rcheevos_hash::{candidates, console_name, hash, hash_console_for};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub enum ListOutcome {
    Available(Vec<GameEntry>),
    Unavailable(MessageCode),
}

pub trait HashListSource {
    fn list(&self, console_id: u32) -> ListOutcome;
    fn game_hashes(&self, game_id: u32) -> Option<Vec<GameHash>>;
}

fn find_game<'a>(games: &'a [GameEntry], md5: &str) -> Option<&'a GameEntry> {
    games
        .iter()
        .find(|g| g.hashes.iter().any(|h| h.eq_ignore_ascii_case(md5)))
}

fn base_result(input: &Input) -> CheckResult {
    CheckResult {
        file_name: input.display_name.clone(),
        source_path: input.source_path.to_string_lossy().into_owned(),
        inner_name: input.inner_name.clone(),
        console_id: None,
        console_name: None,
        md5: None,
        status: Status::Error,
        game: None,
        matched_name: None,
        game_hashes: Vec::new(),
        alternatives: Vec::new(),
        message: None,
    }
}

fn error_result(input: &Input, code: MessageCode) -> CheckResult {
    CheckResult { message: Some(code), ..base_result(input) }
}

/// Nombre usado para detectar la extensión: el del interior del zip o la ruta.
fn name_path(input: &Input) -> PathBuf {
    match &input.inner_name {
        Some(n) => PathBuf::from(n),
        None => input.source_path.clone(),
    }
}

/// Archivos comprimidos que la app no abre (7z es un no-objetivo). rcheevos mapea `.7z` a Arcade
/// (hash del nombre), lo que daría un "No registrado" falso para una ROM comprimida.
const UNSUPPORTED_ARCHIVES: [&str; 2] = ["7z", "rar"];

/// Imágenes de disco comprimidas que rcheevos no lee (solo `.iso`, `.cue`...): GameCube y
/// Wii (Dolphin), PSP y PS2 (`.cso`/`.zso`). `.wbfs` sí se lee (`rcheevos_hash`, módulo `wbfs`).
const COMPRESSED_DISCS: [&str; 6] = ["rvz", "gcz", "wia", "ciso", "cso", "zso"];

/// Fila previa común de `check` y `check_as`: disco dentro de zip o archivo comprimido.
fn early_result(input: &Input, path: &Path) -> Option<CheckResult> {
    if input.disc_in_zip {
        return Some(error_result(input, MessageCode::DiscInZip));
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let message = if UNSUPPORTED_ARCHIVES.contains(&ext.as_str()) {
        MessageCode::ArchiveNotSupported
    } else if COMPRESSED_DISCS.contains(&ext.as_str()) {
        MessageCode::CompressedDisc
    } else if ext == "chd" {
        // rcheevos lista `.chd`, pero esta compilación no incluye lector de CHD: el hash fallaría.
        MessageCode::ChdNotSupported
    } else {
        return None;
    };
    Some(CheckResult { status: Status::Unsupported, message: Some(message), ..base_result(input) })
}

fn matched_result(
    input: &Input,
    console_id: u32,
    md5: &str,
    game: &GameEntry,
    src: &dyn HashListSource,
) -> CheckResult {
    let game_hashes = src.game_hashes(game.id).unwrap_or_default();
    let matched_name = game_hashes
        .iter()
        .find(|h| h.md5.eq_ignore_ascii_case(md5))
        .map(|h| h.name.clone());
    CheckResult {
        console_id: Some(console_id),
        console_name: Some(console_name(console_id).to_string()),
        md5: Some(md5.to_string()),
        status: if game.num_achievements == 0 {
            Status::RegisteredNoAchievements
        } else {
            Status::Registered
        },
        game: Some(game.clone()),
        matched_name,
        game_hashes,
        ..base_result(input)
    }
}

fn unmatched_result(
    input: &Input,
    console_id: u32,
    md5: &str,
    unavailable: Option<MessageCode>,
    alternatives: Vec<(u32, String)>,
) -> CheckResult {
    CheckResult {
        console_id: Some(console_id),
        console_name: Some(console_name(console_id).to_string()),
        md5: Some(md5.to_string()),
        status: if unavailable.is_some() { Status::Unchecked } else { Status::NotRegistered },
        alternatives,
        message: unavailable,
        ..base_result(input)
    }
}

pub fn check(input: &Input, src: &dyn HashListSource) -> CheckResult {
    let path = name_path(input);
    if let Some(r) = early_result(input, &path) {
        return r;
    }
    let data = input.data.as_deref();
    let candidates = candidates(&path, data);
    if candidates.is_empty() {
        return CheckResult { status: Status::Unsupported, ..base_result(input) };
    }

    // Un hash por método: las consolas que comparten método reutilizan el MD5 (`hash_as`).
    let mut by_method: HashMap<u32, Option<String>> = HashMap::new();
    // (consola, md5, motivo si su lista no estuvo disponible)
    let mut hashed: Vec<(u32, String, Option<MessageCode>)> = Vec::new();
    for cand in candidates {
        let c = cand.console_id;
        let md5 = by_method
            .entry(cand.hash_as)
            .or_insert_with(|| hash(&path, data, cand.hash_as));
        let Some(h) = md5.clone() else { continue };
        match src.list(c) {
            ListOutcome::Available(games) => {
                if let Some(game) = find_game(&games, &h) {
                    return matched_result(input, c, &h, game, src);
                }
                hashed.push((c, h, None));
            }
            ListOutcome::Unavailable(code) => hashed.push((c, h, Some(code))),
        }
    }
    let Some((c0, h0, unavailable)) = hashed.first().cloned() else {
        return error_result(input, MessageCode::NoHash);
    };
    let alternatives = hashed[1..]
        .iter()
        .map(|(c, _, _)| (*c, console_name(*c).to_string()))
        .collect();
    unmatched_result(input, c0, &h0, unavailable, alternatives)
}

pub fn check_as(input: &Input, console_id: u32, src: &dyn HashListSource) -> CheckResult {
    let path = name_path(input);
    if let Some(r) = early_result(input, &path) {
        return r;
    }
    let data = input.data.as_deref();
    let Some(h) = hash(&path, data, hash_console_for(&path, data, console_id)) else {
        return error_result(input, MessageCode::NoHash);
    };
    match src.list(console_id) {
        ListOutcome::Available(games) => match find_game(&games, &h) {
            Some(game) => matched_result(input, console_id, &h, game, src),
            None => unmatched_result(input, console_id, &h, None, Vec::new()),
        },
        ListOutcome::Unavailable(code) => {
            unmatched_result(input, console_id, &h, Some(code), Vec::new())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeSource {
        lists: HashMap<u32, Vec<GameEntry>>,
        unavailable: Option<MessageCode>,
        hashes: HashMap<u32, Vec<GameHash>>,
        asked: RefCell<Vec<u32>>,
    }

    impl HashListSource for FakeSource {
        fn list(&self, console_id: u32) -> ListOutcome {
            self.asked.borrow_mut().push(console_id);
            if let Some(code) = self.unavailable {
                return ListOutcome::Unavailable(code);
            }
            ListOutcome::Available(self.lists.get(&console_id).cloned().unwrap_or_default())
        }
        fn game_hashes(&self, game_id: u32) -> Option<Vec<GameHash>> {
            self.hashes.get(&game_id).cloned()
        }
    }

    fn input(name: &str, data: &[u8]) -> Input {
        Input {
            source_path: PathBuf::from(name),
            display_name: name.to_string(),
            inner_name: None,
            data: Some(data.to_vec()),
            disc_in_zip: false,
        }
    }

    fn game(id: u32, console: u32, ach: u32, md5: &str) -> GameEntry {
        GameEntry {
            id,
            title: "Juego".into(),
            console_id: console,
            num_achievements: ach,
            hashes: vec![md5.to_uppercase()],
        }
    }

    fn md5_of(name: &str, data: &[u8], console: u32) -> String {
        rcheevos_hash::hash(Path::new(name), Some(data), console).unwrap()
    }

    const GBA_DATA: &[u8] = b"gba rom bytes for testing";

    fn gba_source(ach: u32) -> (FakeSource, String) {
        let h = md5_of("a.gba", GBA_DATA, 5);
        let mut s = FakeSource::default();
        s.lists.insert(5, vec![game(42, 5, ach, &h)]);
        (s, h)
    }

    #[test]
    fn gba_match_is_registered() {
        let (s, h) = gba_source(10);
        let r = check(&input("a.gba", GBA_DATA), &s);
        assert_eq!(r.status, Status::Registered);
        assert_eq!(r.game.unwrap().id, 42);
        assert_eq!(r.console_id, Some(5));
        assert_eq!(r.md5, Some(h));
    }

    #[test]
    fn zero_achievements() {
        let (s, _) = gba_source(0);
        let r = check(&input("a.gba", GBA_DATA), &s);
        assert_eq!(r.status, Status::RegisteredNoAchievements);
    }

    #[test]
    fn no_match_is_not_registered() {
        let mut s = FakeSource::default();
        s.lists.insert(5, vec![game(1, 5, 3, "ffffffffffffffffffffffffffffffff")]);
        let r = check(&input("a.gba", GBA_DATA), &s);
        assert_eq!(r.status, Status::NotRegistered);
        assert!(r.md5.is_some());
        assert!(r.game.is_none());
    }

    #[test]
    fn no_list_is_unchecked() {
        let s = FakeSource { unavailable: Some(MessageCode::NoApiKey), ..Default::default() };
        let r = check(&input("a.gba", GBA_DATA), &s);
        assert_eq!(r.status, Status::Unchecked);
        assert_eq!(r.message, Some(MessageCode::NoApiKey));
        assert!(r.md5.is_some());
    }

    /// Candidatas que rcheevos realmente hashea para un `.dsk` arbitrario (el `.bin` solo hashea como Mega Drive).
    fn bin_hashed() -> (Vec<u8>, Vec<(u32, String)>) {
        let data: Vec<u8> = (0..4096u32).map(|i| (i * 7 + 3) as u8).collect();
        let hashed: Vec<(u32, String)> =
            rcheevos_hash::candidate_consoles(Path::new("x.dsk"), Some(&data))
                .into_iter()
                .filter_map(|c| {
                    rcheevos_hash::hash(Path::new("x.dsk"), Some(&data), c).map(|h| (c, h))
                })
                .collect();
        (data, hashed)
    }

    #[test]
    fn ambiguous_stops_at_first_match() {
        let (data, hashed) = bin_hashed();
        assert!(hashed.len() >= 2, "fixture needs >= 2 hashed candidates: {hashed:?}");
        let mut s = FakeSource::default();
        s.lists.insert(hashed[1].0, vec![game(7, hashed[1].0, 5, &hashed[1].1)]);
        let r = check(&input("x.dsk", &data), &s);
        assert_eq!(r.status, Status::Registered);
        assert_eq!(r.console_id, Some(hashed[1].0));
        assert_eq!(*s.asked.borrow(), vec![hashed[0].0, hashed[1].0]);
    }

    #[test]
    fn ambiguous_no_match_uses_first() {
        let (data, hashed) = bin_hashed();
        assert!(hashed.len() >= 2);
        let s = FakeSource::default();
        let r = check(&input("x.dsk", &data), &s);
        assert_eq!(r.status, Status::NotRegistered);
        assert_eq!(r.console_id, Some(hashed[0].0));
        assert_eq!(r.md5.as_deref(), Some(hashed[0].1.as_str()));
        let alt_ids: Vec<u32> = r.alternatives.iter().map(|a| a.0).collect();
        let want: Vec<u32> = hashed[1..].iter().map(|h| h.0).collect();
        assert_eq!(alt_ids, want);
    }

    #[test]
    fn matched_name_from_game_hashes() {
        let (mut s, h) = gba_source(10);
        s.hashes.insert(
            42,
            vec![
                GameHash { md5: "0".repeat(32), name: "Otro".into(), labels: vec![] },
                GameHash { md5: h.to_uppercase(), name: "Juego (USA)".into(), labels: vec![] },
            ],
        );
        let r = check(&input("a.gba", GBA_DATA), &s);
        assert_eq!(r.matched_name.as_deref(), Some("Juego (USA)"));
    }

    #[test]
    fn matched_result_includes_all_game_hashes() {
        let (mut s, h) = gba_source(10);
        s.hashes.insert(
            42,
            vec![
                GameHash { md5: "0".repeat(32), name: "Otro".into(), labels: vec![] },
                GameHash { md5: h.to_uppercase(), name: "Juego (USA)".into(), labels: vec!["nointro".into()] },
            ],
        );
        let r = check(&input("a.gba", GBA_DATA), &s);
        assert_eq!(r.game_hashes.len(), 2);
        assert_eq!(r.game_hashes[1].labels, vec!["nointro".to_string()]);
    }

    #[test]
    fn unmatched_result_has_no_game_hashes() {
        let s = FakeSource::default();
        let r = check(&input("a.gba", GBA_DATA), &s);
        assert!(r.game_hashes.is_empty());
    }

    #[test]
    fn unknown_extension_is_unsupported() {
        let s = FakeSource::default();
        let r = check(&input("notes.txt", b"hola"), &s);
        assert_eq!(r.status, Status::Unsupported);
        assert!(s.asked.borrow().is_empty());
    }

    #[test]
    fn zip_with_cue_flags_disc() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("disc.zip");
        std::fs::write(&p, crate::inputs::tests::make_zip(&[("juego.cue", b"x"), ("juego.bin", b"y")]))
            .unwrap();
        let inputs = crate::inputs::expand(&p).unwrap();
        let s = FakeSource::default();
        for i in &inputs {
            let r = check(i, &s);
            assert_eq!(r.status, Status::Error);
            assert_eq!(r.message, Some(MessageCode::DiscInZip));
        }
    }

    /// 16 sectors of 2048 bytes whose sector 0 starts with the Saturn volume header.
    fn saturn_iso(dir: &tempfile::TempDir) -> PathBuf {
        let mut iso: Vec<u8> = (0..2048 * 16u32).map(|i| (i * 13 + 1) as u8).collect();
        iso[..16].copy_from_slice(b"SEGA SEGASATURN ");
        let p = dir.path().join("Saturn Game (Japón).iso");
        std::fs::write(&p, iso).unwrap();
        p
    }

    fn file_input(p: &Path) -> Input {
        Input {
            source_path: p.to_path_buf(),
            display_name: p.file_name().unwrap().to_string_lossy().into_owned(),
            inner_name: None,
            data: None,
            disc_in_zip: false,
        }
    }

    #[test]
    fn saturn_disc_matches_saturn_list_via_sega_cd_hash() {
        let dir = tempfile::tempdir().unwrap();
        let p = saturn_iso(&dir);
        let h = rcheevos_hash::hash(&p, None, 9).unwrap();
        let mut s = FakeSource::default();
        s.lists.insert(39, vec![game(77, 39, 12, &h)]);
        let r = check(&file_input(&p), &s);
        assert_eq!(r.status, Status::Registered);
        assert_eq!(r.console_id, Some(39));
        assert_eq!(r.console_name.as_deref(), Some("Sega Saturn"));
        assert_eq!(r.md5.as_deref(), Some(h.as_str()));
        assert_eq!(*s.asked.borrow(), vec![9, 39]);
    }

    #[test]
    fn saturn_offered_in_selector_and_check_as_works() {
        let dir = tempfile::tempdir().unwrap();
        let p = saturn_iso(&dir);
        let h = rcheevos_hash::hash(&p, None, 9).unwrap();
        let r = check(&file_input(&p), &FakeSource::default());
        assert_eq!(r.status, Status::NotRegistered);
        assert_eq!(r.console_id, Some(9));
        assert!(r.alternatives.iter().any(|a| a.0 == 39), "{:?}", r.alternatives);
        let mut s = FakeSource::default();
        s.lists.insert(39, vec![game(77, 39, 12, &h)]);
        let r = check_as(&file_input(&p), 39, &s);
        assert_eq!(r.status, Status::Registered);
        assert_eq!(r.console_id, Some(39));
        assert_eq!(*s.asked.borrow(), vec![39]);
    }

    #[test]
    fn bin_cartridge_matches_atari_2600_list() {
        let data: Vec<u8> = (0..4096u32).map(|i| (i * 5 + 9) as u8).collect();
        let h = md5_of("a.bin", &data, 1);
        let mut s = FakeSource::default();
        s.lists.insert(25, vec![game(3, 25, 20, &h)]);
        let r = check(&input("Pitfall.bin", &data), &s);
        assert_eq!(r.status, Status::Registered);
        assert_eq!(r.console_id, Some(25));
        assert_eq!(*s.asked.borrow(), vec![1, 10, 25]);
    }

    #[test]
    fn check_as_thomson_rom_uses_msx_hash() {
        let data = b"thomson to8 cartridge";
        let h = md5_of("a.rom", data, 29);
        let mut s = FakeSource::default();
        s.lists.insert(66, vec![game(4, 66, 2, &h)]);
        let r = check_as(&input("jeu.rom", data), 66, &s);
        assert_eq!(r.status, Status::Registered);
        assert_eq!(r.console_id, Some(66));
    }

    #[test]
    fn wonderswan_ws_is_supported() {
        let data = b"wonderswan rom";
        let mut s = FakeSource::default();
        let h = md5_of("x.gb", data, 4); // whole-file MD5
        s.lists.insert(53, vec![game(8, 53, 1, &h)]);
        let r = check(&input("x.ws", data), &s);
        assert_eq!(r.status, Status::Registered);
        assert_eq!(r.console_id, Some(53));
    }

    #[test]
    fn arcade_zip_checks_filename_hash() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sf2.zip");
        std::fs::write(&p, crate::inputs::tests::make_zip(&[("sf2_30a.11e", b"a"), ("sf2.03", b"b")]))
            .unwrap();
        let inputs = crate::inputs::expand(&p).unwrap();
        assert_eq!(inputs.len(), 1);
        let h = md5_of("name.gb", b"sf2", 4); // rcheevos' arcade hash: MD5 of "sf2"
        let mut s = FakeSource::default();
        s.lists.insert(27, vec![game(9, 27, 30, &h)]);
        let r = check(&inputs[0], &s);
        assert_eq!(r.status, Status::Registered);
        assert_eq!(r.console_id, Some(27));
        assert_eq!(r.md5.as_deref(), Some(h.as_str()));
        assert_eq!(r.file_name, "sf2.zip");
        assert_eq!(check_as(&inputs[0], 27, &s).status, Status::Registered);
    }

    #[test]
    fn compressed_discs_ask_to_convert() {
        let s = FakeSource::default();
        for name in ["Super Mario Galaxy.rvz", "Galaxy.GCZ", "game.cso"] {
            let r = check(&input(name, b"x"), &s);
            assert_eq!(r.status, Status::Unsupported, "{name}");
            assert_eq!(r.message, Some(MessageCode::CompressedDisc), "{name}");
            assert_eq!(check_as(&input(name, b"x"), 19, &s).message, Some(MessageCode::CompressedDisc));
        }
    }

    #[test]
    fn wbfs_is_hashed_as_wii() {
        // Un archivo que no es WBFS de verdad: se intenta como Wii (no se pide convertirlo) y,
        // al no poder leerse, da "sin hash".
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Galaxy.wbfs");
        std::fs::write(&p, b"not really wbfs").unwrap();
        let r = check(&file_input(&p), &FakeSource::default());
        assert_eq!(r.status, Status::Error);
        assert_eq!(r.message, Some(MessageCode::NoHash));
    }

    #[test]
    fn chd_asks_to_extract() {
        let s = FakeSource::default();
        let r = check(&input("Game (USA).CHD", b"x"), &s);
        assert_eq!(r.status, Status::Unsupported);
        assert_eq!(r.message, Some(MessageCode::ChdNotSupported));
    }

    #[test]
    fn zip_bin_cartridge_hashes_as_mega_drive() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sonic.zip");
        let rom: Vec<u8> = (0..2048u32).map(|i| (i * 3) as u8).collect();
        std::fs::write(&p, crate::inputs::tests::make_zip(&[("Sonic.bin", &rom)])).unwrap();
        let inputs = crate::inputs::expand(&p).unwrap();
        let h = md5_of("x.md", &rom, 1);
        let mut s = FakeSource::default();
        s.lists.insert(1, vec![game(1, 1, 40, &h)]);
        let r = check(&inputs[0], &s);
        assert_eq!(r.status, Status::Registered);
        assert_eq!(r.console_id, Some(1));
        assert_eq!(r.message, None);
    }

    #[test]
    fn archives_are_unsupported_with_message() {
        for name in ["Pokemon Emerald.7z", "juego.RAR"] {
            let s = FakeSource::default();
            let r = check(&input(name, b"7z\xbc\xaf\x27\x1c"), &s);
            assert_eq!(r.status, Status::Unsupported, "{name}");
            assert_eq!(r.message, Some(MessageCode::ArchiveNotSupported), "{name}");
            assert!(r.md5.is_none() && r.console_id.is_none(), "{name}");
            assert!(s.asked.borrow().is_empty(), "{name}");
            let r = check_as(&input(name, b"x"), 27, &s);
            assert_eq!(r.message, Some(MessageCode::ArchiveNotSupported), "{name} check_as");
        }
        assert_eq!(
            serde_json::to_string(&MessageCode::ArchiveNotSupported).unwrap(),
            "\"archiveNotSupported\""
        );
    }

    #[test]
    fn check_as_hashes_only_that_console() {
        let (s, _) = gba_source(10);
        let r = check_as(&input("a.gba", GBA_DATA), 5, &s);
        assert_eq!(r.status, Status::Registered);
        assert_eq!(*s.asked.borrow(), vec![5]);
    }
}

