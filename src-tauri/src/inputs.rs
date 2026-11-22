//! Expansión de las rutas soltadas en entradas comprobables (los `.zip` se abren en memoria).

use crate::model::MessageCode;
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct Input {
    pub source_path: PathBuf,
    pub display_name: String,
    pub inner_name: Option<String>,
    pub data: Option<Vec<u8>>,
    pub disc_in_zip: bool,
}

/// Extensiones que siempre son de disco.
const DISC_EXTENSIONS: [&str; 6] = ["cue", "iso", "gdi", "chd", "m3u", "wbfs"];

/// rcheevos trata un `.bin` de más de 32 MB como pista de CD y uno menor como cartucho de
/// Mega Drive y familia (rcheevos v12.5.0, src/rhash/hash.c:1057-1075).
const BIN_DISC_MIN: u64 = 32 * 1024 * 1024;

fn extension_lower(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn expand(path: &Path) -> Result<Vec<Input>, MessageCode> {
    let meta = std::fs::metadata(path).map_err(|_| MessageCode::ReadError)?;
    if meta.is_dir() {
        return Err(MessageCode::ReadError);
    }
    let display_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    if extension_lower(&display_name) != "zip" {
        return Ok(vec![Input {
            source_path: path.to_path_buf(),
            display_name,
            inner_name: None,
            data: None,
            disc_in_zip: false,
        }]);
    }

    let file = std::fs::File::open(path).map_err(|_| MessageCode::ReadError)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| MessageCode::ReadError)?;
    // Un `.bin` junto a un `.cue`/`.gdi`/... es una pista del disco, no un cartucho.
    let has_disc_sheet = archive
        .file_names()
        .any(|n| n.is_ok_and(|n| DISC_EXTENSIONS.contains(&extension_lower(&n).as_str())));
    let mut inputs = Vec::new();
    let mut has_cartridge = false;
    for idx in 0..archive.len() {
        let mut entry = archive.by_index(idx).map_err(|_| MessageCode::ReadError)?;
        if entry.is_dir() {
            continue;
        }
        let inner = entry.name().map_err(|_| MessageCode::ReadError)?.into_owned();
        let ext = extension_lower(&inner);
        let disc = DISC_EXTENSIONS.contains(&ext.as_str())
            || (ext == "bin" && (has_disc_sheet || entry.size() > BIN_DISC_MIN));
        let data = if disc {
            None
        } else {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|_| MessageCode::ReadError)?;
            has_cartridge |= !rcheevos_hash::candidates(Path::new(&inner), Some(&buf)).is_empty();
            Some(buf)
        };
        inputs.push(Input {
            source_path: path.to_path_buf(),
            display_name: display_name.clone(),
            inner_name: Some(inner),
            data,
            disc_in_zip: disc,
        });
    }
    if !inputs.is_empty() && !has_cartridge && !inputs.iter().any(|i| i.disc_in_zip) {
        // Sin cartuchos ni discos reconocidos: un set de arcade, que RA identifica por el
        // nombre del zip (rcheevos lo hashea como Arcade a partir de la ruta).
        return Ok(vec![Input {
            source_path: path.to_path_buf(),
            display_name,
            inner_name: None,
            data: None,
            disc_in_zip: false,
        }]);
    }
    Ok(inputs)
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    pub(crate) fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            for (name, data) in files {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn directory_is_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(expand(dir.path()).err(), Some(MessageCode::ReadError));
    }

    #[test]
    fn zip_two_roms_two_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("pack.zip");
        std::fs::write(&p, make_zip(&[("a.gba", b"aaaa"), ("sub/b.gb", b"bbbb")])).unwrap();
        let inputs = expand(&p).unwrap();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].inner_name.as_deref(), Some("a.gba"));
        assert_eq!(inputs[0].data.as_deref(), Some(&b"aaaa"[..]));
        assert_eq!(inputs[1].inner_name.as_deref(), Some("sub/b.gb"));
        assert_eq!(inputs[1].data.as_deref(), Some(&b"bbbb"[..]));
        assert!(inputs.iter().all(|i| !i.disc_in_zip && i.source_path == p));
    }

    #[test]
    fn zip_with_cue_flags_disc() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("disc.zip");
        std::fs::write(&p, make_zip(&[("juego.cue", b"x"), ("juego.bin", b"y")])).unwrap();
        let inputs = expand(&p).unwrap();
        assert_eq!(inputs.len(), 2);
        assert!(inputs.iter().all(|i| i.disc_in_zip && i.data.is_none()));
    }

    #[test]
    fn zip_with_wbfs_flags_disc() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("wii.zip");
        std::fs::write(&p, make_zip(&[("Galaxy.WBFS", b"x")])).unwrap();
        let inputs = expand(&p).unwrap();
        assert!(inputs[0].disc_in_zip && inputs[0].data.is_none());
    }

    #[test]
    fn zip_without_cartridge_is_one_arcade_input() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sf2.zip");
        std::fs::write(&p, make_zip(&[("sf2_30a.11e", b"rom1"), ("sf2.03", b"rom2")])).unwrap();
        let inputs = expand(&p).unwrap();
        assert_eq!(inputs.len(), 1);
        let i = &inputs[0];
        assert!(i.inner_name.is_none() && i.data.is_none() && !i.disc_in_zip);
        assert_eq!(i.source_path, p);
        assert_eq!(i.display_name, "sf2.zip");
    }

    #[test]
    fn zip_small_bin_is_cartridge() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sonic.zip");
        std::fs::write(&p, make_zip(&[("Sonic.bin", b"SEGA GENESIS rom")])).unwrap();
        let inputs = expand(&p).unwrap();
        assert_eq!(inputs.len(), 1);
        assert!(!inputs[0].disc_in_zip);
        assert_eq!(inputs[0].data.as_deref(), Some(&b"SEGA GENESIS rom"[..]));
    }

    #[test]
    fn zip_bin_next_to_cue_stays_disc_track() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("disc.zip");
        std::fs::write(&p, make_zip(&[("Track 02.bin", b"audio"), ("game.CUE", b"FILE")])).unwrap();
        let inputs = expand(&p).unwrap();
        assert_eq!(inputs.len(), 2);
        assert!(inputs.iter().all(|i| i.disc_in_zip && i.data.is_none()));
    }

    #[test]
    fn zip_big_bin_is_disc() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("disc.zip");
        let big = vec![0u8; 32 * 1024 * 1024 + 1];
        std::fs::write(&p, make_zip(&[("track.bin", &big)])).unwrap();
        let inputs = expand(&p).unwrap();
        assert_eq!(inputs.len(), 1);
        assert!(inputs[0].disc_in_zip && inputs[0].data.is_none());
    }

    #[test]
    fn plain_file_and_bad_zip() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("x.gba");
        std::fs::write(&rom, b"abc").unwrap();
        let inputs = expand(&rom).unwrap();
        assert_eq!(inputs.len(), 1);
        assert!(inputs[0].data.is_none() && inputs[0].inner_name.is_none());
        assert_eq!(inputs[0].display_name, "x.gba");
        let bad = dir.path().join("bad.zip");
        std::fs::write(&bad, b"not a zip").unwrap();
        assert_eq!(expand(&bad).err(), Some(MessageCode::ReadError));
    }
}
