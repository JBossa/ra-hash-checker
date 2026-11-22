//! Reads a `.wbfs` (Wii Backup File System) file as the Wii disc image it stores.
//!
//! Layout (libwbfs, also what Dolphin's `WbfsFileReader` reads):
//! - At 0: `"WBFS"`, `n_hd_sec` (u32 BE), `hd_sec_sz_s` (u8), `wbfs_sec_sz_s` (u8).
//! - At `1 << hd_sec_sz_s`: info of the first disc: a 0x100-byte copy of the disc header, then the
//!   `wlba` table, one u16 BE per `1 << wbfs_sec_sz_s` block of the disc.
//! - Disc offset `p` lives in block `p >> wbfs_sec_sz_s`; its `wlba` entry is the block index in
//!   the file (0 = not stored, read as zeros).
//!
//! Files larger than a FAT32 volume allows are split as `x.wbfs`, `x.wbf1`, `x.wbf2`... and read
//! as one concatenation.
//!
//! rcheevos only knows how to hash Wii discs from a plain image, so [`hash`](crate::hash)
//! installs the C callbacks at the end of this file as the iterator's file reader.

use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"WBFS";
/// Wii discs are addressed in 32 KB sectors; a dual-layer disc has 143432 * 2 of them.
const WII_SECTOR_SHIFT: u8 = 15;
const WII_SECTORS_PER_DISC: u64 = 143_432 * 2;
const DISC_HEADER_COPY: u64 = 0x100;
/// Split parts after the first one: `.wbf1` ... `.wbf9`.
const MAX_SPLIT_PARTS: u32 = 9;

thread_local! {
    /// Blocks read as zeros because the file does not store them, while [`track_missing`] runs.
    /// rcheevos calls the reader synchronously on the hashing thread.
    static MISSING: RefCell<Option<Vec<usize>>> = const { RefCell::new(None) };
}

/// Runs `f` and returns its result with the blocks it read that the `.wbfs` does not store.
pub(crate) fn track_missing<R>(f: impl FnOnce() -> R) -> (R, Vec<usize>) {
    MISSING.with(|m| *m.borrow_mut() = Some(Vec::new()));
    let r = f();
    let missing = MISSING.with(|m| m.borrow_mut().take()).unwrap_or_default();
    (r, missing)
}

fn note_missing(block: usize) {
    MISSING.with(|m| {
        if let Some(v) = m.borrow_mut().as_mut() {
            if !v.contains(&block) {
                v.push(block);
            }
        }
    });
}

struct Part {
    file: File,
    start: u64,
    len: u64,
}

pub(crate) struct WbfsReader {
    parts: Vec<Part>,
    block_shift: u8,
    wlba: Vec<u16>,
    pos: u64,
}

/// `x.wbfs` -> `x.wbf1`, `x.wbf2`... (the last character becomes the part number).
fn split_part_path(path: &Path, n: u32) -> Option<PathBuf> {
    let s = path.to_str()?;
    let mut base = s.to_string();
    base.pop()?;
    Some(PathBuf::from(format!("{base}{n}")))
}

fn open_parts(path: &Path) -> Option<Vec<Part>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut add = |file: File| -> Option<()> {
        let len = file.metadata().ok()?.len();
        parts.push(Part { file, start, len });
        start += len;
        Some(())
    };
    add(File::open(path).ok()?)?;
    for n in 1..=MAX_SPLIT_PARTS {
        match split_part_path(path, n).and_then(|p| File::open(p).ok()) {
            Some(f) => add(f)?,
            None => break,
        }
    }
    Some(parts)
}

impl WbfsReader {
    /// `None` if the file cannot be opened or is not a WBFS file.
    pub(crate) fn open(path: &Path) -> Option<WbfsReader> {
        let parts = open_parts(path)?;
        let mut r = WbfsReader { parts, block_shift: 0, wlba: Vec::new(), pos: 0 };
        let mut head = [0u8; 10];
        if r.read_file(0, &mut head) != head.len() || &head[..4] != MAGIC {
            return None;
        }
        let hd_shift = head[8];
        let block_shift = head[9];
        // Real files use 512-byte disc sectors and 2 MB blocks; reject what cannot be a WBFS.
        if !(9..=16).contains(&hd_shift) || !(WII_SECTOR_SHIFT..=32).contains(&block_shift) {
            return None;
        }
        let blocks = (WII_SECTORS_PER_DISC >> (block_shift - WII_SECTOR_SHIFT)) as usize;
        let mut table = vec![0u8; blocks * 2];
        let table_at = (1u64 << hd_shift) + DISC_HEADER_COPY;
        if r.read_file(table_at, &mut table) != table.len() {
            return None;
        }
        r.wlba = (0..blocks).map(|i| u16::from_be_bytes([table[2 * i], table[2 * i + 1]])).collect();
        r.block_shift = block_shift;
        Some(r)
    }

    fn file_len(&self) -> u64 {
        self.parts.last().map_or(0, |p| p.start + p.len)
    }

    /// Reads `buf.len()` bytes at `pos` of the concatenated files; returns how many were read.
    fn read_file(&mut self, mut pos: u64, mut buf: &mut [u8]) -> usize {
        let mut done = 0;
        while !buf.is_empty() {
            let Some(part) = self.parts.iter_mut().find(|p| pos >= p.start && pos < p.start + p.len) else {
                break;
            };
            let within = pos - part.start;
            let n = buf.len().min((part.len - within) as usize);
            if part.file.seek(SeekFrom::Start(within)).is_err() || part.file.read_exact(&mut buf[..n]).is_err() {
                break;
            }
            done += n;
            pos += n as u64;
            buf = &mut buf[n..];
        }
        done
    }

    /// Size of the disc image, as rcheevos would see it for an `.iso`.
    pub(crate) fn disc_len(&self) -> u64 {
        WII_SECTORS_PER_DISC << WII_SECTOR_SHIFT
    }

    /// Fills `buf` with the disc bytes at `pos`; blocks the file does not store read as zeros.
    pub(crate) fn read_at(&mut self, mut pos: u64, mut buf: &mut [u8]) {
        let block_size = 1u64 << self.block_shift;
        while !buf.is_empty() {
            let within = pos & (block_size - 1);
            let n = buf.len().min((block_size - within) as usize);
            let (chunk, rest) = buf.split_at_mut(n);
            let block = (pos >> self.block_shift) as usize;
            let lba = self.wlba.get(block).copied().unwrap_or(0);
            chunk.fill(0);
            if lba == 0 {
                note_missing(block);
            } else {
                let at = ((lba as u64) << self.block_shift) + within;
                if at < self.file_len() {
                    self.read_file(at, chunk);
                }
            }
            pos += n as u64;
            buf = rest;
        }
    }
}

fn be32(b: &[u8]) -> u64 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64
}

/// `[start, end)` of every update partition (type 1) in the disc's partition table
/// (same fields rc_hash_wii reads, hash_disc.c:1268-1300).
fn update_partitions(r: &mut WbfsReader) -> Vec<(u64, u64)> {
    let mut info = [0u8; 32];
    r.read_at(0x40000, &mut info);
    let mut out = Vec::new();
    for group in info.chunks(8) {
        let (count, table) = (be32(group).min(64), be32(&group[4..]) << 2);
        for i in 0..count {
            let mut entry = [0u8; 8];
            r.read_at(table + i * 8, &mut entry);
            if be32(&entry[4..]) != 1 {
                continue;
            }
            let start = be32(&entry) << 2;
            let mut data = [0u8; 8];
            r.read_at(start + 0x2B8, &mut data);
            out.push((start, start + (be32(&data) << 2) + (be32(&data[4..]) << 2)));
        }
    }
    out
}

/// Whether the blocks the hash read as zeros (`missing`) are the update partition the tool that
/// made the `.wbfs` dropped. rc_hash_wii hashes disc bytes from 0x20400 on (it treats the game
/// partition's data offset as absolute), which on retail discs is the update partition: without
/// it the MD5 cannot match. Missing blocks outside it (unused space, zeros on the disc) are fine.
/// A table that lists no update partition means it was removed along with its data.
pub(crate) fn update_partition_lost(path: &Path, missing: &[usize]) -> bool {
    if missing.is_empty() {
        return false;
    }
    let Some(mut r) = WbfsReader::open(path) else { return false };
    let updates = update_partitions(&mut r);
    if updates.is_empty() {
        return true;
    }
    let size = 1u64 << r.block_shift;
    missing.iter().any(|&b| {
        let (start, end) = (b as u64 * size, (b as u64 + 1) * size);
        updates.iter().any(|&(us, ue)| start < ue && us < end)
    })
}

/// True for paths whose extension is `.wbfs` (any case).
pub(crate) fn is_wbfs(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("wbfs"))
}

// --- rc_hash_filereader_t callbacks -----------------------------------------------------------
// rcheevos owns the handle between open and close; every callback tolerates a null handle.

const SEEK_SET: c_int = 0;
const SEEK_CUR: c_int = 1;
const SEEK_END: c_int = 2;

pub(crate) unsafe extern "C" fn open(path_utf8: *const c_char) -> *mut c_void {
    if path_utf8.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: rcheevos passes the NUL-terminated path it was given.
    let Ok(path) = unsafe { CStr::from_ptr(path_utf8) }.to_str() else {
        return std::ptr::null_mut();
    };
    match WbfsReader::open(Path::new(path)) {
        Some(r) => Box::into_raw(Box::new(r)).cast(),
        None => std::ptr::null_mut(),
    }
}

/// SAFETY: `handle` is null or a pointer returned by [`open`] and not yet closed.
unsafe fn reader<'a>(handle: *mut c_void) -> Option<&'a mut WbfsReader> {
    unsafe { handle.cast::<WbfsReader>().as_mut() }
}

pub(crate) unsafe extern "C" fn seek(handle: *mut c_void, offset: i64, origin: c_int) {
    let Some(r) = (unsafe { reader(handle) }) else { return };
    let base = match origin {
        SEEK_SET => 0,
        SEEK_CUR => r.pos as i64,
        SEEK_END => r.disc_len() as i64,
        _ => return,
    };
    r.pos = base.saturating_add(offset).max(0) as u64;
}

pub(crate) unsafe extern "C" fn tell(handle: *mut c_void) -> i64 {
    unsafe { reader(handle) }.map_or(0, |r| r.pos as i64)
}

pub(crate) unsafe extern "C" fn read(handle: *mut c_void, buffer: *mut c_void, requested: usize) -> usize {
    let Some(r) = (unsafe { reader(handle) }) else { return 0 };
    if buffer.is_null() {
        return 0;
    }
    let n = requested.min(r.disc_len().saturating_sub(r.pos) as usize);
    // SAFETY: rcheevos passes a buffer of at least `requested` bytes.
    let buf = unsafe { std::slice::from_raw_parts_mut(buffer.cast::<u8>(), n) };
    let pos = r.pos;
    r.read_at(pos, buf);
    r.pos += n as u64;
    n
}

pub(crate) unsafe extern "C" fn close(handle: *mut c_void) {
    if !handle.is_null() {
        // SAFETY: `handle` came from Box::into_raw in `open` and rcheevos closes it once.
        drop(unsafe { Box::from_raw(handle.cast::<WbfsReader>()) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_names() {
        assert_eq!(split_part_path(Path::new("C:/g/Game.wbfs"), 1), Some(PathBuf::from("C:/g/Game.wbf1")));
        assert_eq!(split_part_path(Path::new("x.WBFS"), 2), Some(PathBuf::from("x.WBF2")));
    }

    #[test]
    fn extension_check_ignores_case() {
        assert!(is_wbfs(Path::new("a.WbFs")));
        assert!(!is_wbfs(Path::new("a.wbf1")));
        assert!(!is_wbfs(Path::new("a.iso")));
    }
}
