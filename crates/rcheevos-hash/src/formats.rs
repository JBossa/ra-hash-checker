//! Supported file extensions grouped by console, for the "supported formats" list in the UI.
//!
//! Derived from the same sources the hashing uses: rcheevos' extension table
//! (`vendor/rcheevos/src/rhash/hash.c:1212-1297`, mirrored in [`RCHEEVOS_EXTENSIONS`] and checked
//! by a test) expanded with [`crate::candidates`], plus the app's own extra extensions.

use crate::expansions::{candidates, EXTRA_EXTENSIONS};
use std::collections::BTreeMap;
use std::path::Path;

/// Extensions in rcheevos' `rc_hash_iterator_ext_handlers` table, in table order.
pub(crate) const RCHEEVOS_EXTENSIONS: [&str; 84] = [
    "2d", "3ds", "3dsx", "7z", "83g", "83p", "a26", "a78", "app", "arduboy", "axf", "bin", "bs", "cart",
    "cas", "cci", "chd", "chf", "cia", "col", "csw", "cue", "cxi", "d64", "d88", "dosz", "dsk", "elf",
    "fd", "fds", "fig", "gb", "gba", "gbc", "gdi", "gg", "hex", "iso", "jag", "k7", "lnx", "m3u", "m5",
    "m7", "md", "min", "mx1", "mx2", "n64", "ndd", "nds", "neo", "nes", "ngc", "nib", "pbp", "pce",
    "pgm", "pzx", "ri", "rom", "sap", "scl", "sfc", "sg", "sgx", "smc", "sms", "sv", "swc", "tap", "tic",
    "trd", "tvc", "tzx", "uze", "v64", "vb", "wad", "wasm", "woz", "wsc", "z64", "zip",
];

/// Not listed per console: `.7z` is rejected by the app (it cannot open it), `.m3u` is a
/// playlist that is hashed as its first disc, and `.chd` needs a CHD reader this build does not
/// include (rcheevos only lists it; hashing fails without one).
const NOT_PER_CONSOLE: [&str; 3] = ["7z", "m3u", "chd"];

/// A `.bin` over 32 MB is treated as a CD track: hash.c:1057-1068 lists 3DO, PlayStation,
/// PlayStation 2 and Sega CD (whose method also covers Saturn, see `expansions`). The table probe
/// below has no file size, so it only sees the cartridge branch.
const DISC_BIN_CONSOLES: [u32; 5] = [43, 12, 21, 9, 39];

/// `(console_id, extensions)` for every console the app can hash, sorted by console id;
/// extensions are lowercase, without the dot, sorted and unique.
pub fn formats_by_console() -> Vec<(u32, Vec<&'static str>)> {
    let mut map: BTreeMap<u32, Vec<&'static str>> = BTreeMap::new();
    let extensions = RCHEEVOS_EXTENSIONS
        .iter()
        .copied()
        .chain(EXTRA_EXTENSIONS.iter().map(|e| e.0))
        .filter(|e| !NOT_PER_CONSOLE.contains(e));
    for ext in extensions {
        let name = format!("file.{ext}");
        // An empty buffer: no file is read, and size-based handlers see a small file.
        for c in candidates(Path::new(&name), Some(&[])) {
            map.entry(c.console_id).or_default().push(ext);
        }
    }
    for id in DISC_BIN_CONSOLES {
        map.entry(id).or_default().push("bin");
    }
    map.into_iter()
        .map(|(id, mut exts)| {
            exts.sort_unstable();
            exts.dedup();
            (id, exts)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exts(id: u32) -> Vec<&'static str> {
        formats_by_console().into_iter().find(|(c, _)| *c == id).map(|(_, e)| e).unwrap_or_default()
    }

    /// The mirrored list must match the vendored table exactly.
    #[test]
    fn mirror_matches_rcheevos_table() {
        let src = include_str!("../vendor/rcheevos/src/rhash/hash.c");
        let start = src.find("rc_hash_iterator_ext_handlers[] = {").unwrap();
        let end = start + src[start..].find("};").unwrap();
        let table: Vec<&str> = src[start..end]
            .lines()
            .filter_map(|l| l.trim().strip_prefix("{ \""))
            .map(|l| &l[..l.find('"').unwrap()])
            .collect();
        assert_eq!(table, RCHEEVOS_EXTENSIONS.to_vec());
    }

    #[test]
    fn known_consoles_have_their_extensions() {
        assert_eq!(exts(19), vec!["iso", "wad", "wbfs"]); // Wii: no .rvz
        assert_eq!(exts(16), vec!["iso"]); // GameCube
        assert!(exts(39).contains(&"cue") && exts(39).contains(&"bin")); // Saturn
        assert_eq!(exts(12), vec!["bin", "cue"]); // PlayStation (no .chd in this build)
        assert!(exts(1).contains(&"md") && exts(1).contains(&"bin")); // Mega Drive
        assert_eq!(exts(53), vec!["ws", "wsc"]); // WonderSwan (app extra + table)
        assert!(exts(27).contains(&"zip") && !exts(27).contains(&"7z")); // Arcade
        assert!(exts(78).contains(&"nds")); // DSi shares the DS method
    }

    #[test]
    fn every_console_is_named_and_sorted() {
        let all = formats_by_console();
        assert!(all.len() > 60);
        for (id, exts) in all {
            assert_ne!(crate::console_name(id), "Unknown", "{id}");
            assert!(!exts.is_empty());
            assert!(exts.windows(2).all(|w| w[0] < w[1]), "{id}: {exts:?}");
        }
    }
}
