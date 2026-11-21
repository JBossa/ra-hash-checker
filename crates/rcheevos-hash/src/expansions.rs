//! App-owned expansion of rcheevos' candidate consoles.
//!
//! rcheevos' extension table names ONE representative console per hash method (for example
//! `.cue` lists Sega CD with the comment "ASSERT: handles both Sega CD and Saturn"). RA keeps a
//! separate hash list per console, so the app must also look the MD5 up in every console that
//! shares the method. Everything here is derived from the vendored rcheevos v12.5.0 sources;
//! line numbers refer to `vendor/rcheevos/src/rhash/hash.c` and console ids to
//! `vendor/rcheevos/include/rc_consoles.h`.

use crate::candidate_consoles;
use std::path::Path;

/// A console to look the file up in, and the console whose rcheevos routine computes its MD5.
/// `hash_as == console_id` means rcheevos hashes that console itself; otherwise the MD5 is the
/// one computed for `hash_as` (same routine, or a console rcheevos does not dispatch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub console_id: u32,
    pub hash_as: u32,
}

const MEGA_DRIVE: u32 = 1;
const SEGA_CD: u32 = 9;
const NINTENDO: u32 = 7;
const NINTENDO_DS: u32 = 18;
const MSX: u32 = 29;
const COMMODORE_64: u32 = 30;
const GAMEBOY: u32 = 4;

/// `(console_id, hash_as)` pairs appended right after the representative `rep` for `ext`.
/// `raw` is rcheevos' own candidate list for the file.
fn expansions(ext: &str, rep: u32, raw: &[u32]) -> &'static [(u32, u32)] {
    match (rep, ext) {
        // Sega CD (9) -> Saturn (39). The `.chd` (hash.c:1084), `.cue` (hash.c:1099) and `.iso`
        // (hash.c:1162) handlers list RC_CONSOLE_SEGA_CD with "ASSERT: handles both Sega CD and
        // Saturn". Large raw `.bin` (hash.c:1064) and `.m3u` (hash.c:1167-1188, resolved to its
        // first disc) reach the same candidate. rc_hash_from_file dispatches both ids to the same
        // code: `case RC_CONSOLE_SEGA_CD: case RC_CONSOLE_SATURN:` -> m3u playlist or
        // rc_hash_sega_cd (hash.c:971-976), which accepts both volume headers
        // (hash_disc.c:1211-1212), so the MD5 is identical.
        (SEGA_CD, "cue" | "iso" | "chd" | "bin" | "m3u") => &[(39, SEGA_CD)],

        // `.bin` up to 32 MB (or buffered): hash.c:1072-1075 "bin is associated with MegaDrive,
        // Sega32X, Atari 2600, Watara Supervision, MegaDuck, Fairchild Channel F, Arcadia 2001,
        // Interton VC 4000, and Super Cassette Vision. Since they all use the same hashing
        // algorithm, only specify one of them". That branch lists Mega Drive alone; the >32 MB
        // branch (hash.c:1060-1068) lists it only as a disc fallback, so it is not expanded.
        // Dispatch: Mega Drive -> rc_hash_whole_file (hash.c:845-850) / rc_hash_buffer
        // (hash.c:538,556); 32X (831), 2600 (814), Supervision (833), MegaDuck (827), Channel F
        // (818), Arcadia (813), VC 4000 (824) -> rc_hash_whole_file (hash.c:843), and the same
        // ids -> rc_hash_buffer (hash.c:523-556): identical MD5, so they reuse Mega Drive's.
        // Super Cassette Vision is dispatched to rc_hash_scv (hash.c:580-581, 857-860), which
        // skips an "EmuSCV" header (hash_rom.c:501-511), so it is hashed with its own id.
        (MEGA_DRIVE, "bin") if raw == [MEGA_DRIVE] => &[
            (10, MEGA_DRIVE), // RC_CONSOLE_SEGA_32X
            (25, MEGA_DRIVE), // RC_CONSOLE_ATARI_2600
            (63, MEGA_DRIVE), // RC_CONSOLE_SUPERVISION
            (69, MEGA_DRIVE), // RC_CONSOLE_MEGADUCK
            (57, MEGA_DRIVE), // RC_CONSOLE_FAIRCHILD_CHANNEL_F
            (73, MEGA_DRIVE), // RC_CONSOLE_ARCADIA_2001
            (74, MEGA_DRIVE), // RC_CONSOLE_INTERTON_VC_4000
            (55, 55),         // RC_CONSOLE_SUPER_CASSETTEVISION (own routine)
        ],

        // `.rom`: hash.c:1200-1202 "rom is associated with MSX, Thomson TO-8, and Fairchild
        // Channel F. Since they all use the same hashing algorithm, only specify one of them".
        // MSX -> whole file (hash.c:865-871, buffer 540,556); Channel F -> whole file (hash.c:818,
        // buffer 528). Thomson TO-8 has no case in rc_hash_from_file/rc_hash_from_buffer
        // (hash.c:502-593, 805-982), so rc_hash_generate(66) fails; it takes MSX's MD5 as the
        // comment states.
        (MSX, "rom") => &[
            (66, MSX), // RC_CONSOLE_THOMSONTO8
            (57, MSX), // RC_CONSOLE_FAIRCHILD_CHANNEL_F
        ],

        // `.tap`: hash.c:1208-1209 "also Oric and ZX Spectrum, but all are full file hashes".
        // C64 -> whole file (hash.c:864-871, buffer 527); Oric (829, buffer 542) and ZX Spectrum
        // (841, buffer 555) -> whole file.
        (COMMODORE_64, "tap") => &[
            (32, COMMODORE_64), // RC_CONSOLE_ORIC
            (59, COMMODORE_64), // RC_CONSOLE_ZX_SPECTRUM
        ],

        // `.nds`: hash.c:1263 "handles both DS and DSi"; `case RC_CONSOLE_NINTENDO_DS: case
        // RC_CONSOLE_NINTENDO_DSI:` share rc_hash_nintendo_ds (hash.c:929-931) and the buffer
        // path (hash.c:589-591).
        (NINTENDO_DS, "nds") => &[(78, NINTENDO_DS)], // RC_CONSOLE_NINTENDO_DSI

        // `.fds`: listed as RC_CONSOLE_NINTENDO (hash.c:1242); `case
        // RC_CONSOLE_FAMICOM_DISK_SYSTEM: case RC_CONSOLE_NINTENDO:` share rc_hash_nes
        // (hash.c:573-575) and the buffered-file path (hash.c:854-860).
        (NINTENDO, "fds") => &[(81, NINTENDO)], // RC_CONSOLE_FAMICOM_DISK_SYSTEM

        _ => &[],
    }
}

/// Standard extensions with no entry in rcheevos' table (hash.c:1212-1297). rcheevos reaches
/// them only through its "No console mapping" fallback, which hashes as Game Boy, a whole-file
/// MD5 (hash.c:1345-1350, 819/843); the app removed that fallback, so it maps them here.
/// `(extension, console_id, hash_as)`.
const EXTRA_EXTENSIONS: [(&str, u32, u32); 9] = [
    // RC_CONSOLE_WONDERSWAN: whole file (hash.c:840, buffer 554). `.wsc` is in the table (1294).
    ("ws", 53, 53),
    // RC_CONSOLE_NEOGEO_POCKET: whole file (hash.c:828, buffer 541). `.ngc` is in the table (1266).
    ("ngp", 14, 14),
    // RC_CONSOLE_SEGA_32X: whole file (hash.c:831, buffer 545).
    ("32x", 10, 10),
    // RC_CONSOLE_ATARI_5200: no case in either dispatch, so it keeps rcheevos' fallback, the
    // Game Boy whole-file hash (hash.c:1349-1350).
    ("a52", 50, GAMEBOY),
    // RC_CONSOLE_VECTREX: whole file (hash.c:837, buffer 551).
    ("vec", 46, 46),
    // RC_CONSOLE_INTELLIVISION: whole file (hash.c:823, buffer 534).
    ("int", 45, 45),
    // RC_CONSOLE_MAGNAVOX_ODYSSEY2: whole file (hash.c:825, buffer 536).
    ("o2", 23, 23),
    // RC_CONSOLE_ATARI_JAGUAR: whole file (hash.c:815, buffer 525).
    ("j64", 17, 17),
    // RC_CONSOLE_GAME_AND_WATCH: no case in either dispatch; Game Boy fallback (hash.c:1349-1350).
    ("mgw", 60, GAMEBOY),
];

fn extension_lower(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Every console to check for this file, in order: each rcheevos candidate followed by the
/// consoles that share its hash method. Empty if the extension is unknown.
/// If `data` is `Some`, `path` only supplies the file name.
pub fn candidates(path: &Path, data: Option<&[u8]>) -> Vec<Candidate> {
    let ext = extension_lower(path);
    if let Some(&(_, console_id, hash_as)) = EXTRA_EXTENSIONS.iter().find(|e| e.0 == ext) {
        return vec![Candidate { console_id, hash_as }];
    }
    let raw = candidate_consoles(path, data);
    let mut out: Vec<Candidate> = Vec::new();
    let mut push = |c: Candidate| {
        if !out.iter().any(|o| o.console_id == c.console_id) {
            out.push(c);
        }
    };
    for &rep in &raw {
        push(Candidate { console_id: rep, hash_as: rep });
        for &(console_id, hash_as) in expansions(&ext, rep, &raw) {
            push(Candidate { console_id, hash_as });
        }
    }
    out
}

/// The console whose rcheevos routine computes `console_id`'s MD5 for this file.
pub fn hash_console_for(path: &Path, data: Option<&[u8]>, console_id: u32) -> u32 {
    candidates(path, data)
        .into_iter()
        .find(|c| c.console_id == console_id)
        .map_or(console_id, |c| c.hash_as)
}
