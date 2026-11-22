//! `.wbfs` (Wii Backup File System) se lee como el `.iso` equivalente.

use rcheevos_hash::{candidates, hash, Candidate};
use std::fs;
use std::path::PathBuf;

const WII: u32 = 19;
/// Bloques WBFS de 256 KB: la tabla (35858 entradas) cabe en el primer bloque.
const SHIFT: u8 = 18;
const UPDATE_PART: usize = 0x40_0000;
const ISO_LEN: usize = 0x48_0000;

fn pseudo(len: usize, seed: u32) -> Vec<u8> {
    let mut s = seed;
    (0..len)
        .map(|_| {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            (s >> 24) as u8
        })
        .collect()
}

fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

/// ISO de Wii mínimo con lo que lee `rc_hash_wii`: cabecera cifrada, tabla de particiones con
/// una partición de juego y una de actualización, TMD y datos de la partición de juego.
fn wii_iso() -> Vec<u8> {
    let mut iso = pseudo(ISO_LEN, 7);
    put32(&mut iso, 0x18, 0x5D1C_9EA3); // magia de disco de Wii
    iso[0x61] = 0; // cifrado: se hashean clústeres en bruto
    // Tabla de particiones: 2 particiones en 0x40020.
    iso[0x40000..0x40020].fill(0);
    put32(&mut iso, 0x40000, 2);
    put32(&mut iso, 0x40004, 0x40020 >> 2);
    put32(&mut iso, 0x40020, 0x50000 >> 2); // juego
    put32(&mut iso, 0x40024, 0);
    put32(&mut iso, 0x40028, (UPDATE_PART >> 2) as u32); // actualización (no se hashea)
    put32(&mut iso, 0x4002C, 1);
    // Partición de juego: TMD y datos (2 MB = 64 clústeres).
    put32(&mut iso, 0x50000 + 0x2A4, 0x208);
    put32(&mut iso, 0x50000 + 0x2A8, 0x2C0 >> 2);
    put32(&mut iso, 0x50000 + 0x2B8, 0x8_0000 >> 2);
    put32(&mut iso, 0x50000 + 0x2BC, 0x20_0000 >> 2);
    // Hueco sin datos entre la partición de juego y la de actualización.
    iso[0x28_0000..UPDATE_PART].fill(0);
    iso
}

/// Empaqueta `iso` como un `.wbfs` de un solo juego (formato de libwbfs). Los bloques vacíos y
/// los de la partición de actualización no se guardan, como hacen las herramientas WBFS.
fn to_wbfs(iso: &[u8]) -> Vec<u8> {
    let sec = 1usize << SHIFT;
    let hd = 512usize;
    let blocks_per_disc = (143_432 * 2) >> (SHIFT - 15);
    let mut out = vec![0u8; sec];
    out[0..4].copy_from_slice(b"WBFS");
    put32(&mut out, 4, 0x1234);
    out[8] = 9; // 512 bytes por sector de disco
    out[9] = SHIFT;
    out[12] = 1; // disc_table[0]: hay un juego
    out[hd..hd + 0x100].copy_from_slice(&iso[..0x100]);
    let table = hd + 0x100;
    let mut next_lba = 1u16;
    for (i, block) in iso.chunks(sec).enumerate() {
        let skip = block.iter().all(|&b| b == 0) || i * sec >= UPDATE_PART;
        if skip {
            continue;
        }
        out[table + i * 2..table + i * 2 + 2].copy_from_slice(&next_lba.to_be_bytes());
        let mut padded = block.to_vec();
        padded.resize(sec, 0);
        out.extend_from_slice(&padded);
        next_lba += 1;
    }
    assert!(table + blocks_per_disc * 2 <= sec);
    out
}

fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.path().join(name);
    fs::write(&p, bytes).unwrap();
    p
}

fn iso_hash(dir: &tempfile::TempDir, iso: &[u8]) -> String {
    let h = hash(&write(dir, "game.iso", iso), None, WII);
    h.expect("el ISO sintético debe tener hash de Wii")
}

#[test]
fn wbfs_hashes_like_the_iso() {
    let d = tempfile::tempdir().unwrap();
    let iso = wii_iso();
    let wbfs = to_wbfs(&iso);
    assert!(wbfs.len() < iso.len(), "el .wbfs de prueba debe omitir bloques");
    let p = write(&d, "game.wbfs", &wbfs);
    assert_eq!(hash(&p, None, WII), Some(iso_hash(&d, &iso)));
}

#[test]
fn split_wbfs_hashes_like_the_iso() {
    let d = tempfile::tempdir().unwrap();
    let iso = wii_iso();
    let wbfs = to_wbfs(&iso);
    // Corte a mitad de un bloque: las lecturas cruzan de una parte a la otra.
    let cut = wbfs.len() / 2 + 12_345;
    let p = write(&d, "game.wbfs", &wbfs[..cut]);
    write(&d, "game.wbf1", &wbfs[cut..]);
    assert_eq!(hash(&p, None, WII), Some(iso_hash(&d, &iso)));
}

#[test]
fn uppercase_extension_works() {
    let d = tempfile::tempdir().unwrap();
    let iso = wii_iso();
    let p = write(&d, "GAME.WBFS", &to_wbfs(&iso));
    assert_eq!(hash(&p, None, WII), Some(iso_hash(&d, &iso)));
}

#[test]
fn not_a_wbfs_file_has_no_hash() {
    let d = tempfile::tempdir().unwrap();
    let p = write(&d, "fake.wbfs", &pseudo(1 << 20, 3));
    assert_eq!(hash(&p, None, WII), None);
    let empty = write(&d, "empty.wbfs", b"");
    assert_eq!(hash(&empty, None, WII), None);
}

#[test]
fn wbfs_is_a_wii_candidate() {
    assert_eq!(
        candidates(std::path::Path::new("Game (USA).wbfs"), None),
        vec![Candidate { console_id: WII, hash_as: WII }]
    );
}
