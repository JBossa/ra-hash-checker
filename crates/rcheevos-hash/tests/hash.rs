use rcheevos_hash::{candidate_consoles, console_name, hash};
use std::fs;
use std::path::{Path, PathBuf};

fn md5_hex(b: &[u8]) -> String {
    format!("{:x}", md5::compute(b))
}

fn pseudo(len: usize, seed: u32) -> Vec<u8> {
    let mut s = seed;
    (0..len)
        .map(|_| {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            (s >> 24) as u8
        })
        .collect()
}

fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.path().join(name);
    fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn gba_is_md5_of_whole_file() {
    let d = tempfile::tempdir().unwrap();
    let all = pseudo(4096, 1);
    let p = write(&d, "x.gba", &all);
    assert_eq!(hash(&p, None, 5), Some(md5_hex(&all)));
}

#[test]
fn gba_candidates() {
    let d = tempfile::tempdir().unwrap();
    let p = write(&d, "x.gba", &pseudo(4096, 1));
    assert_eq!(candidate_consoles(&p, None), vec![5]);
}

#[test]
fn gb_gbc_genesis_whole_file() {
    let d = tempfile::tempdir().unwrap();
    let all = pseudo(4096, 2);
    for (name, id) in [("x.gb", 4u32), ("x.gbc", 6), ("x.md", 1)] {
        let p = write(&d, name, &all);
        assert_eq!(hash(&p, None, id), Some(md5_hex(&all)), "{name}");
        assert!(candidate_consoles(&p, None).contains(&id), "{name}");
    }
}

#[test]
fn nes_skips_ines_header() {
    let d = tempfile::tempdir().unwrap();
    let data = pseudo(32768, 3);
    let mut all = b"NES\x1a".to_vec();
    all.extend_from_slice(&[0u8; 12]);
    all.extend_from_slice(&data);
    let p = write(&d, "x.nes", &all);
    assert_eq!(hash(&p, None, 7), Some(md5_hex(&data)));
}

#[test]
fn snes_skips_copier_header() {
    let d = tempfile::tempdir().unwrap();
    let data = pseudo(8192 * 4, 4);
    let mut all = vec![0u8; 512];
    all.extend_from_slice(&data);
    assert_eq!(all.len() % 8192, 512);
    let p = write(&d, "x.sfc", &all);
    assert_eq!(hash(&p, None, 3), Some(md5_hex(&data)));
}

#[test]
fn n64_v64_equals_z64() {
    let d = tempfile::tempdir().unwrap();
    let mut z64 = pseudo(8192, 5);
    z64[..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
    let mut v64 = z64.clone();
    for pair in v64.chunks_exact_mut(2) {
        pair.swap(0, 1);
    }
    let pz = write(&d, "x.z64", &z64);
    let pv = write(&d, "x.v64", &v64);
    let hz = hash(&pz, None, 2);
    assert!(hz.is_some());
    assert_eq!(hash(&pv, None, 2), hz);
}

#[test]
fn buffer_input_matches_file() {
    let d = tempfile::tempdir().unwrap();
    let bytes = pseudo(4096, 6);
    let p = write(&d, "file.gba", &bytes);
    let from_file = hash(&p, None, 5);
    assert!(from_file.is_some());
    assert_eq!(hash(Path::new("inner.gba"), Some(&bytes), 5), from_file);
}

#[test]
fn hash_non_ascii_path() {
    let d = tempfile::tempdir().unwrap();
    let bytes = pseudo(4096, 7);
    let plain = write(&d, "plain.gba", &bytes);
    let fancy = write(&d, "Pokémon Esmeralda (España).gba", &bytes);
    let expected = hash(&plain, None, 5);
    assert!(expected.is_some());
    assert_eq!(hash(&fancy, None, 5), expected);
}

#[test]
fn unknown_extension_has_no_candidates() {
    let d = tempfile::tempdir().unwrap();
    let p = write(&d, "notes.txt", b"hello");
    assert_eq!(candidate_consoles(&p, None), Vec::<u32>::new());
}

#[test]
fn console_names() {
    assert_eq!(console_name(5), "Game Boy Advance");
    assert_eq!(console_name(12), "PlayStation");
    assert_eq!(console_name(9999), "Unknown");
}

#[test]
#[ignore]
fn ps1_bloody_roar_2() {
    let cue = std::env::var("RA_PS1_CUE").expect("set RA_PS1_CUE");
    let p = PathBuf::from(cue);
    assert!(candidate_consoles(&p, None).contains(&12));
    assert_eq!(hash(&p, None, 12).as_deref(), Some("77defab4cedf9f82c4251a474d55cbee"));
}
