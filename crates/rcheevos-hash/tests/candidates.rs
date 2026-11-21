//! Expansion of rcheevos' representative candidates to every RA console sharing the hash method.

use rcheevos_hash::{candidates, hash, Candidate};
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

fn ids(c: &[Candidate]) -> Vec<u32> {
    c.iter().map(|c| c.console_id).collect()
}

fn find(c: &[Candidate], id: u32) -> Candidate {
    *c.iter().find(|c| c.console_id == id).unwrap_or_else(|| panic!("{id} missing in {c:?}"))
}

/// 16 sectors of 2048 bytes whose sector 0 starts with the Saturn volume header.
fn saturn_iso() -> Vec<u8> {
    let mut iso = pseudo(2048 * 16, 11);
    iso[..16].copy_from_slice(b"SEGA SEGASATURN ");
    iso
}

#[test]
fn cue_iso_chd_expand_sega_cd_to_saturn() {
    for name in ["x.cue", "x.iso", "x.chd", "X.CUE"] {
        let c = candidates(Path::new(name), None);
        let v = ids(&c);
        let i = v.iter().position(|&id| id == 9).unwrap_or_else(|| panic!("{name}: {v:?}"));
        assert_eq!(v.get(i + 1), Some(&39), "{name}: Saturn right after Sega CD: {v:?}");
        assert_eq!(find(&c, 39).hash_as, 9, "{name}");
    }
}

#[test]
fn saturn_disc_hashes_like_sega_cd() {
    let d = tempfile::tempdir().unwrap();
    let p = write(&d, "saturn.iso", &saturn_iso());
    let via_sega_cd = hash(&p, None, 9);
    assert!(via_sega_cd.is_some());
    assert_eq!(hash(&p, None, 39), via_sega_cd);
}

#[test]
fn small_bin_expands_to_whole_file_family() {
    let d = tempfile::tempdir().unwrap();
    let data = pseudo(4096, 1);
    let p = write(&d, "x.bin", &data);
    for (path, buf) in [(p.as_path(), None), (Path::new("inner.bin"), Some(data.as_slice()))] {
        let c = candidates(path, buf);
        assert_eq!(ids(&c), vec![1, 10, 25, 63, 69, 57, 73, 74, 55], "{path:?}");
        for id in [1, 10, 25, 63, 69, 57, 73, 74] {
            assert_eq!(find(&c, id).hash_as, 1);
        }
        assert_eq!(find(&c, 55).hash_as, 55, "SCV has its own routine (EmuSCV header)");
    }
}

#[test]
fn rom_tap_nds_fds_expansions() {
    let r = candidates(Path::new("x.rom"), None);
    assert_eq!(ids(&r), vec![29, 66, 57]);
    assert!(r.iter().all(|c| c.hash_as == 29));
    let t = candidates(Path::new("x.tap"), None);
    assert_eq!(ids(&t), vec![30, 32, 59]);
    assert!(t.iter().all(|c| c.hash_as == 30));
    let n = candidates(Path::new("x.nds"), None);
    assert_eq!(ids(&n), vec![18, 78]);
    assert!(n.iter().all(|c| c.hash_as == 18));
    let f = candidates(Path::new("x.fds"), None);
    assert_eq!(ids(&f), vec![7, 81]);
    assert!(f.iter().all(|c| c.hash_as == 7));
}

/// Every expansion that reuses another console's MD5 must get exactly the MD5 rcheevos
/// computes for that console itself (when rcheevos can hash it at all).
#[test]
fn shared_md5_equals_rcheevos_own_hash() {
    let d = tempfile::tempdir().unwrap();
    let data = pseudo(8192, 3);
    let mut nds = pseudo(4096, 4);
    nds[0x20..0x24].copy_from_slice(&0x200u32.to_le_bytes()); // arm9 addr
    nds[0x2C..0x30].copy_from_slice(&0x100u32.to_le_bytes()); // arm9 size
    nds[0x30..0x34].copy_from_slice(&0x300u32.to_le_bytes()); // arm7 addr
    nds[0x3C..0x40].copy_from_slice(&0x100u32.to_le_bytes()); // arm7 size
    nds[0x68..0x6C].copy_from_slice(&0u32.to_le_bytes()); // no icon
    let mut fds = b"FDS\x1a".to_vec();
    fds.extend_from_slice(&[0u8; 12]);
    fds.extend_from_slice(&pseudo(65500, 5));
    let files = [
        write(&d, "a.bin", &data),
        write(&d, "a.rom", &data),
        write(&d, "a.tap", &data),
        write(&d, "a.nds", &nds),
        write(&d, "a.fds", &fds),
        write(&d, "a.iso", &saturn_iso()),
    ];
    let mut checked = 0;
    for p in &files {
        for c in candidates(p, None) {
            if c.hash_as == c.console_id {
                continue;
            }
            let shared = hash(p, None, c.hash_as);
            assert!(shared.is_some(), "{p:?} via {}", c.hash_as);
            if let Some(own) = hash(p, None, c.console_id) {
                assert_eq!(Some(own), shared, "{p:?} console {}", c.console_id);
                checked += 1;
            }
        }
    }
    assert!(checked >= 13, "only {checked} expansions verified");
}

#[test]
fn thomson_to8_is_not_dispatched_by_rcheevos() {
    // hash.c v12.5.0 has no RC_CONSOLE_THOMSONTO8 case in rc_hash_from_file/rc_hash_from_buffer,
    // which is why `.rom` reaches TO8 through MSX's routine (hash.c:1200-1201).
    let data = pseudo(1024, 6);
    assert_eq!(hash(Path::new("a.rom"), Some(&data), 66), None);
    assert_eq!(hash(Path::new("a.rom"), Some(&data), 29), Some(md5_hex(&data)));
}

#[test]
fn big_bin_expands_only_sega_cd() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("big.bin");
    let f = fs::File::create(&p).unwrap();
    f.set_len(33 * 1024 * 1024).unwrap();
    drop(f);
    assert_eq!(ids(&candidates(&p, None)), vec![43, 12, 21, 9, 39, 1]);
}

#[test]
fn extra_extensions_hash_whole_file() {
    let d = tempfile::tempdir().unwrap();
    let data = pseudo(4096, 8);
    for (name, id) in [
        ("x.ws", 53u32),
        ("x.ngp", 14),
        ("x.32x", 10),
        ("x.a52", 50),
        ("x.vec", 46),
        ("x.int", 45),
        ("x.o2", 23),
        ("x.j64", 17),
        ("x.mgw", 60),
        ("X.WS", 53),
    ] {
        let p = write(&d, name, &data);
        let c = candidates(&p, None);
        assert_eq!(ids(&c), vec![id], "{name}");
        assert_eq!(hash(&p, None, c[0].hash_as), Some(md5_hex(&data)), "{name}");
        assert_eq!(
            hash(Path::new(name), Some(&data), c[0].hash_as),
            Some(md5_hex(&data)),
            "{name} buffered"
        );
    }
}

#[test]
fn unexpanded_and_unknown_are_unchanged() {
    assert_eq!(ids(&candidates(Path::new("x.gba"), None)), vec![5]);
    assert!(candidates(Path::new("x.gba"), None).iter().all(|c| c.hash_as == c.console_id));
    assert!(candidates(Path::new("notes.txt"), None).is_empty());
    assert!(candidates(Path::new("noext"), None).is_empty());
}
