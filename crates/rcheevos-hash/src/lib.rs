//! Safe wrapper over the rcheevos hashing code (iterator API only).

mod consoles;
mod expansions;
mod formats;
mod wbfs;

pub use consoles::console_name;
pub use expansions::{candidates, hash_console_for, Candidate};
pub use formats::formats_by_console;

/// Why [`hash_checked`] has no hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashError {
    /// rcheevos could not hash the input for that console.
    Failed,
    /// A `.wbfs` without the disc's update partition: the RetroAchievements Wii hash covers it,
    /// so no hash computed from this file can match.
    WbfsUpdatePartitionRemoved,
}

use std::ffi::{c_char, c_int, c_void, CString};
use std::marker::PhantomData;
use std::path::Path;
use std::ptr;

type MessageFn = Option<unsafe extern "C" fn(*const c_char, *const RcHashIterator)>;

/// `rc_hash_filereader_t`: open, seek, tell, read, close (RC_CCONV is cdecl, i.e. "C").
#[repr(C)]
struct FileReader {
    open: Option<unsafe extern "C" fn(*const c_char) -> *mut c_void>,
    seek: Option<unsafe extern "C" fn(*mut c_void, i64, c_int)>,
    tell: Option<unsafe extern "C" fn(*mut c_void) -> i64>,
    read: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> usize>,
    close: Option<unsafe extern "C" fn(*mut c_void)>,
}

/// `rc_hash_cdreader_t` (RC_HASH_NO_DISC is not defined): open_track, read_sector, close_track, first_track_sector, open_track_iterator.
#[repr(C)]
struct CdReader {
    _fns: [*const c_void; 5],
}

/// `rc_hash_callbacks_t` compiled with RC_HASH_NO_ENCRYPTED (no `encryption` block).
#[repr(C)]
struct Callbacks {
    _verbose_message: MessageFn,
    _error_message: MessageFn,
    filereader: FileReader,
    _cdreader: CdReader,
}

/// `rc_hash_iterator_t`.
#[repr(C)]
struct RcHashIterator {
    buffer: *const u8,
    buffer_size: usize,
    consoles: [u8; 12],
    index: c_int,
    path: *const c_char,
    userdata: *mut c_void,
    callbacks: Callbacks,
}

extern "C" {
    fn rc_hash_initialize_iterator(
        it: *mut RcHashIterator,
        path: *const c_char,
        buffer: *const u8,
        buffer_size: usize,
    );
    fn rc_hash_destroy_iterator(it: *mut RcHashIterator);
    fn rc_hash_generate(hash: *mut c_char, console_id: u32, it: *const RcHashIterator) -> c_int;
    fn rch_populate_candidates(it: *mut RcHashIterator) -> c_int;
}

/// Owns an initialized iterator and always destroys it. The path CString and
/// the data slice are borrowed for `'a`, so they outlive the iterator.
struct Iter<'a> {
    raw: RcHashIterator,
    _marker: PhantomData<&'a [u8]>,
}

impl<'a> Iter<'a> {
    fn new(path: &'a CString, data: Option<&'a [u8]>) -> Self {
        // SAFETY: zeroed is a valid state for raw pointers, integers and Option<fn>;
        // rc_hash_initialize_iterator then fills in every field.
        let mut raw: RcHashIterator = unsafe { std::mem::zeroed() };
        let (buf, len) = match data {
            Some(d) => (d.as_ptr(), d.len()),
            None => (ptr::null(), 0),
        };
        unsafe { rc_hash_initialize_iterator(&mut raw, path.as_ptr(), buf, len) };
        Iter { raw, _marker: PhantomData }
    }
}

impl Drop for Iter<'_> {
    fn drop(&mut self) {
        unsafe { rc_hash_destroy_iterator(&mut self.raw) };
    }
}

fn to_cstring(path: &Path) -> Option<CString> {
    CString::new(path.to_str()?).ok()
}

/// Console ids rcheevos considers for this file, in rcheevos' order.
/// Empty if the extension is unknown or the path is not valid UTF-8.
/// If `data` is `Some`, `path` only supplies the file name.
pub fn candidate_consoles(path: &Path, data: Option<&[u8]>) -> Vec<u32> {
    let Some(c_path) = to_cstring(path) else {
        return Vec::new();
    };
    let mut it = Iter::new(&c_path, data);
    // SAFETY: `it` is initialized and live; this only fills `consoles` (and, for .m3u,
    // swaps `path` for an allocation that rc_hash_destroy_iterator frees).
    if unsafe { rch_populate_candidates(&mut it.raw) } == 0 {
        return Vec::new();
    }
    it.raw
        .consoles
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u32)
        .collect()
}

/// Lowercase 32-char MD5 as computed by RetroAchievements for `console_id`,
/// or `None` if rcheevos cannot hash the input (see [`hash_checked`] for why).
/// If `data` is `Some`, `path` only supplies the file name.
pub fn hash(path: &Path, data: Option<&[u8]>, console_id: u32) -> Option<String> {
    hash_checked(path, data, console_id).ok()
}

/// Like [`hash`], but says why there is no hash.
pub fn hash_checked(path: &Path, data: Option<&[u8]>, console_id: u32) -> Result<String, HashError> {
    let c_path = to_cstring(path).ok_or(HashError::Failed)?;
    let mut it = Iter::new(&c_path, data);
    if data.is_none() && wbfs::is_wbfs(path) {
        // Present the .wbfs to rcheevos as the disc image it contains.
        it.raw.callbacks.filereader = FileReader {
            open: Some(wbfs::open),
            seek: Some(wbfs::seek),
            tell: Some(wbfs::tell),
            read: Some(wbfs::read),
            close: Some(wbfs::close),
        };
        let (md5, missing) = wbfs::track_missing(|| generate(&it, console_id));
        if wbfs::update_partition_lost(path, &missing) {
            return Err(HashError::WbfsUpdatePartitionRemoved);
        }
        return md5.ok_or(HashError::Failed);
    }
    generate(&it, console_id).ok_or(HashError::Failed)
}

fn generate(it: &Iter<'_>, console_id: u32) -> Option<String> {
    let mut out = [0 as c_char; 33];
    // SAFETY: `out` holds the 33 bytes rcheevos writes; `it` is live and initialized.
    let ok = unsafe { rc_hash_generate(out.as_mut_ptr(), console_id, &it.raw) };
    if ok == 0 {
        return None;
    }
    let bytes: Vec<u8> = out
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" {
        fn rch_iterator_sizeof() -> usize;
    }

    #[test]
    fn layout_matches() {
        assert_eq!(std::mem::size_of::<RcHashIterator>(), unsafe {
            rch_iterator_sizeof()
        });
    }
}
