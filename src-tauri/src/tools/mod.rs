//! Herramientas externas que el usuario tiene instaladas y que convierten formatos que la app no
//! lee por sí misma. Por ahora, DolphinTool (`.rvz`, `.gcz`, `.wia` → `.iso`).
//!
//! Cada herramienta es un [`ToolSpec`]; añadir otra es añadir una entrada a [`TOOLS`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub struct ToolSpec {
    pub id: &'static str,
    pub name: &'static str,
    /// Nombres del ejecutable en este sistema.
    pub exe_names: &'static [&'static str],
    /// Extensiones (minúsculas, sin punto) que la herramienta permite comprobar.
    pub extensions: &'static [&'static str],
    /// Consolas con las que se compara la imagen convertida.
    pub consoles: &'static [u32],
    pub download_url: &'static str,
    /// Textos (minúsculas) que la salida debe contener al ejecutarla sin argumentos.
    pub signature: &'static [&'static str],
    /// Prefijo (minúsculas) de las carpetas donde se busca en la detección automática.
    pub folder_prefix: &'static str,
    /// Tamaño máximo de la salida, para cuando la cabecera de la entrada no lo dice.
    pub max_output: u64,
    /// Argumentos para convertir `input` en `output`, con `work` como carpeta de trabajo.
    pub convert_args: fn(input: &Path, output: &Path, work: &Path) -> Vec<OsString>,
    /// Extensión de la imagen que produce.
    pub output_ext: &'static str,
}

const GAMECUBE: u32 = 16;
const WII: u32 = 19;
/// Disco de Wii de doble capa.
const WII_DUAL_LAYER: u64 = 8_511_160_320;

fn dolphin_args(input: &Path, output: &Path, work: &Path) -> Vec<OsString> {
    let mut v: Vec<OsString> = Vec::new();
    for (flag, value) in [("-i", input), ("-o", output), ("-u", work)] {
        v.push(flag.into());
        v.push(value.into());
    }
    let mut args: Vec<OsString> = vec!["convert".into(), "-f".into(), "iso".into()];
    args.extend(v);
    args
}

#[cfg(windows)]
const DOLPHIN_EXES: &[&str] = &["DolphinTool.exe"];
#[cfg(not(windows))]
const DOLPHIN_EXES: &[&str] = &["dolphin-tool"];

pub static DOLPHIN: ToolSpec = ToolSpec {
    id: "dolphin",
    name: "Dolphin (DolphinTool)",
    exe_names: DOLPHIN_EXES,
    extensions: &["rvz", "gcz", "wia"],
    consoles: &[GAMECUBE, WII],
    download_url: "https://dolphin-emu.org/download/",
    signature: &["commands supported", "convert"],
    folder_prefix: "dolphin",
    max_output: WII_DUAL_LAYER,
    convert_args: dolphin_args,
    output_ext: "iso",
};

pub static TOOLS: [&ToolSpec; 1] = [&DOLPHIN];

pub fn tool_by_id(id: &str) -> Option<&'static ToolSpec> {
    TOOLS.iter().copied().find(|t| t.id == id)
}

pub fn tool_for_extension(ext: &str) -> Option<&'static ToolSpec> {
    let ext = ext.to_ascii_lowercase();
    TOOLS.iter().copied().find(|t| t.extensions.contains(&ext.as_str()))
}

pub fn tool_for_path(path: &Path) -> Option<&'static ToolSpec> {
    tool_for_extension(&path.extension()?.to_string_lossy())
}

// --- Configuración ---------------------------------------------------------------------------

/// Ruta del ejecutable de cada herramienta, por id (`tools.json`).
#[derive(Default, Serialize, Deserialize, Debug, PartialEq)]
pub struct ToolsConfig(BTreeMap<String, PathBuf>);

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ToolStatus {
    Connected,
    /// Hay una ruta guardada pero el archivo ya no existe.
    Missing,
    NotConfigured,
}

impl ToolsConfig {
    /// Configuración vacía si el archivo no existe o está corrupto.
    pub fn load(file: &Path) -> ToolsConfig {
        std::fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(file, serde_json::to_vec_pretty(self)?)
    }

    pub fn path_for(&self, id: &str) -> Option<&Path> {
        self.0.get(id).map(PathBuf::as_path)
    }

    pub fn set(&mut self, id: &str, exe: PathBuf) {
        self.0.insert(id.to_string(), exe);
    }

    pub fn clear(&mut self, id: &str) {
        self.0.remove(id);
    }

    pub fn status(&self, id: &str) -> ToolStatus {
        match self.path_for(id) {
            None => ToolStatus::NotConfigured,
            Some(p) if p.is_file() => ToolStatus::Connected,
            Some(_) => ToolStatus::Missing,
        }
    }

    /// Ejecutable listo para usar (configurado y presente).
    pub fn usable(&self, id: &str) -> Option<&Path> {
        (self.status(id) == ToolStatus::Connected).then(|| self.path_for(id)).flatten()
    }
}

// --- Procesos --------------------------------------------------------------------------------

fn command(exe: &Path) -> Command {
    let mut c = Command::new(exe);
    c.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

const VALIDATE_TIMEOUT: Duration = Duration::from_secs(5);

/// Ejecuta `exe` sin argumentos y comprueba que su salida sea la de la herramienta.
pub fn validate(spec: &ToolSpec, exe: &Path) -> bool {
    let Ok(mut child) = command(exe).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() else {
        return false;
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < VALIDATE_TIMEOUT => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
    let mut out = String::new();
    if let Some(mut s) = child.stdout.take() {
        let _ = s.read_to_string(&mut out);
    }
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_string(&mut out);
    }
    let out = out.to_lowercase();
    spec.signature.iter().all(|s| out.contains(s))
}

// --- Detección -------------------------------------------------------------------------------

/// Busca el ejecutable en `dir` y en sus subcarpetas que empiezan por `prefix`, hasta `depth`
/// niveles (para las versiones portables descomprimidas en Descargas, etc.).
fn find_in(dir: &Path, spec: &ToolSpec, depth: u32) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = spec.exe_names.iter().map(|n| dir.join(n)).filter(|p| p.is_file()).collect();
    if depth == 0 {
        return found;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return found };
    let mut subdirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| e.file_name().to_string_lossy().to_lowercase().starts_with(spec.folder_prefix))
        .map(|e| e.path())
        .collect();
    subdirs.sort();
    for d in subdirs {
        found.extend(find_in(&d, spec, depth - 1));
    }
    found
}

fn candidates(spec: &ToolSpec) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            out.extend(find_in(&dir, spec, 0));
        }
    }
    #[cfg(windows)]
    {
        let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
        // Instalaciones: carpetas `Dolphin*` directamente bajo estas rutas.
        let installs = [
            env("ProgramFiles"),
            env("ProgramFiles(x86)"),
            env("LOCALAPPDATA").map(|p| p.join("Programs")),
        ];
        for dir in installs.into_iter().flatten() {
            out.extend(find_in(&dir, spec, 1));
        }
        if let Some(home) = env("USERPROFILE") {
            out.extend(find_in(&home.join("scoop").join("apps").join("dolphin").join("current"), spec, 0));
            // Versiones portables: hasta dos niveles de carpetas `Dolphin*`.
            for sub in ["Downloads", "Desktop", "Documents"] {
                out.extend(find_in(&home.join(sub), spec, 2));
            }
        }
    }
    out
}

/// Primer ejecutable encontrado que pasa [`validate`].
pub fn detect(spec: &ToolSpec) -> Option<PathBuf> {
    candidates(spec).into_iter().find(|p| validate(spec, p))
}

// --- Conversión ------------------------------------------------------------------------------

/// Tamaño de la imagen que produce la conversión, leído de la cabecera de `input`:
/// RVZ/WIA (`iso_file_size`, u64 BE en 0x24) o GCZ (`data_size`, u64 LE en 0x10).
pub fn output_size(input: &Path) -> Option<u64> {
    let mut head = [0u8; 0x2C];
    std::fs::File::open(input).ok()?.read_exact(&mut head).ok()?;
    let u64_at = |at: usize| -> [u8; 8] { head[at..at + 8].try_into().unwrap_or_default() };
    if &head[..4] == b"RVZ\x01" || &head[..4] == b"WIA\x01" {
        return Some(u64::from_be_bytes(u64_at(0x24)));
    }
    if u32::from_le_bytes([head[0], head[1], head[2], head[3]]) == 0xB10B_C001 {
        return Some(u64::from_le_bytes(u64_at(0x10)));
    }
    None
}

/// Bytes libres en el volumen de `dir` (`None` si no se puede saber).
pub fn free_space(dir: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut avail = 0u64;
        // SAFETY: `wide` es una cadena terminada en 0 y `avail` vive durante la llamada.
        let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, std::ptr::null_mut(), std::ptr::null_mut()) };
        (ok != 0).then_some(avail)
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        None
    }
}

/// Margen además del tamaño de la imagen.
const SPACE_MARGIN: u64 = 64 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum ToolError {
    /// Faltan bytes libres; lleva los necesarios.
    NoSpace(u64),
    Failed,
}

/// Imagen convertida en la carpeta temporal; se borra (con su carpeta de trabajo) al soltarla.
pub struct Converted {
    pub path: PathBuf,
    work: PathBuf,
}

impl Drop for Converted {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir_all(&self.work);
    }
}

fn unique_stem() -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    format!("{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed))
}

/// Convierte `input` con la herramienta en `tmp`. Comprueba antes el espacio libre.
pub fn convert(spec: &ToolSpec, exe: &Path, input: &Path, tmp: &Path) -> Result<Converted, ToolError> {
    std::fs::create_dir_all(tmp).map_err(|_| ToolError::Failed)?;
    let needed = output_size(input).unwrap_or(spec.max_output) + SPACE_MARGIN;
    if let Some(free) = free_space(tmp) {
        if free < needed {
            return Err(ToolError::NoSpace(needed));
        }
    }
    let stem = unique_stem();
    let out = Converted { path: tmp.join(format!("{stem}.{}", spec.output_ext)), work: tmp.join(format!("{stem}-work")) };
    let status = command(exe)
        .args((spec.convert_args)(input, &out.path, &out.work))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() && out.path.is_file() => Ok(out),
        _ => Err(ToolError::Failed),
    }
}

/// Vacía la carpeta temporal (restos de una sesión cerrada a mitad de una conversión).
pub fn clean_tmp(tmp: &Path) {
    let _ = std::fs::remove_dir_all(tmp);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn extensions_map_to_dolphin() {
        assert_eq!(tool_for_extension("RVZ").map(|t| t.id), Some("dolphin"));
        assert_eq!(tool_for_path(Path::new("a/Galaxy.wia")).map(|t| t.id), Some("dolphin"));
        assert!(tool_for_extension("iso").is_none());
        assert!(tool_by_id("dolphin").is_some() && tool_by_id("nope").is_none());
    }

    #[test]
    fn output_size_reads_rvz_wia_and_gcz_headers() {
        let d = tempfile::tempdir().unwrap();
        let mut rvz = vec![0u8; 0x40];
        rvz[..4].copy_from_slice(b"RVZ\x01");
        rvz[0x24..0x2C].copy_from_slice(&4_699_979_776u64.to_be_bytes());
        assert_eq!(output_size(&write(d.path(), "a.rvz", &rvz)), Some(4_699_979_776));
        rvz[..4].copy_from_slice(b"WIA\x01");
        assert_eq!(output_size(&write(d.path(), "a.wia", &rvz)), Some(4_699_979_776));
        let mut gcz = vec![0u8; 0x40];
        gcz[..4].copy_from_slice(&0xB10B_C001u32.to_le_bytes());
        gcz[0x10..0x18].copy_from_slice(&1_459_978_240u64.to_le_bytes());
        assert_eq!(output_size(&write(d.path(), "a.gcz", &gcz)), Some(1_459_978_240));
    }

    #[test]
    fn output_size_rejects_unknown_or_short_files() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(output_size(&write(d.path(), "x.rvz", &[0u8; 0x40])), None);
        assert_eq!(output_size(&write(d.path(), "short.rvz", b"RVZ\x01")), None);
        assert_eq!(output_size(&d.path().join("missing.rvz")), None);
    }

    #[test]
    fn config_roundtrip_and_status() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("cfg").join("tools.json");
        assert_eq!(ToolsConfig::load(&file).status("dolphin"), ToolStatus::NotConfigured);
        let exe = write(d.path(), "DolphinTool.exe", b"x");
        let mut c = ToolsConfig::default();
        c.set("dolphin", exe.clone());
        c.save(&file).unwrap();
        let loaded = ToolsConfig::load(&file);
        assert_eq!(loaded.status("dolphin"), ToolStatus::Connected);
        assert_eq!(loaded.usable("dolphin"), Some(exe.as_path()));
        std::fs::remove_file(&exe).unwrap();
        assert_eq!(loaded.status("dolphin"), ToolStatus::Missing);
        assert_eq!(loaded.usable("dolphin"), None);
        let mut cleared = loaded;
        cleared.clear("dolphin");
        assert_eq!(cleared.status("dolphin"), ToolStatus::NotConfigured);
    }

    #[test]
    fn corrupt_config_is_empty() {
        let d = tempfile::tempdir().unwrap();
        let file = write(d.path(), "tools.json", b"{not json");
        assert_eq!(ToolsConfig::load(&file), ToolsConfig::default());
    }

    #[test]
    fn finds_portable_builds_two_levels_down() {
        let d = tempfile::tempdir().unwrap();
        let exe = DOLPHIN.exe_names[0];
        let portable = write(d.path(), &format!("dolphin-2609b-x64/Dolphin-x64/{exe}"), b"x");
        write(d.path(), &format!("other/Dolphin-x64/{exe}"), b"x"); // carpeta sin el prefijo
        write(d.path(), &format!("dolphin-a/dolphin-b/dolphin-c/{exe}"), b"x"); // demasiado profundo
        assert_eq!(find_in(d.path(), &DOLPHIN, 2), vec![portable]);
    }

    #[test]
    fn a_program_that_is_not_dolphin_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        assert!(!validate(&DOLPHIN, &d.path().join("missing.exe")));
        assert!(!validate(&DOLPHIN, Path::new("cargo")));
    }

    #[cfg(windows)]
    #[test]
    fn a_dolphintool_lookalike_is_accepted() {
        let d = tempfile::tempdir().unwrap();
        let fake = write(d.path(), "fake.bat", b"@echo usage: dolphin-tool COMMAND -h\r\n@echo commands supported: [convert, verify, header, extract]\r\n");
        assert!(validate(&DOLPHIN, &fake));
    }

    #[cfg(windows)]
    #[test]
    fn convert_runs_the_tool_and_cleans_up() {
        let d = tempfile::tempdir().unwrap();
        let input = write(d.path(), "game.rvz", b"rvz");
        // Argumentos: convert -f iso -i IN -o OUT -u WORK  ->  OUT es %7.
        let ok = write(d.path(), "ok.bat", b"@echo converted> \"%~7\"\r\n");
        let tmp = d.path().join("tmp");
        let out = convert(&DOLPHIN, &ok, &input, &tmp).unwrap();
        let iso = out.path.clone();
        assert_eq!(std::fs::read_to_string(&iso).unwrap().trim(), "converted");
        drop(out);
        assert!(!iso.exists());
    }

    #[cfg(windows)]
    #[test]
    fn failed_conversion_leaves_nothing_behind() {
        let d = tempfile::tempdir().unwrap();
        let input = write(d.path(), "game.rvz", b"rvz");
        let fail = write(d.path(), "fail.bat", b"@echo partial> \"%~7\"\r\n@exit /b 1\r\n");
        let tmp = d.path().join("tmp");
        assert_eq!(convert(&DOLPHIN, &fail, &input, &tmp).err(), Some(ToolError::Failed));
        assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0);
    }

    #[test]
    fn not_enough_space_is_reported_before_running() {
        let d = tempfile::tempdir().unwrap();
        let mut rvz = vec![0u8; 0x40];
        rvz[..4].copy_from_slice(b"RVZ\x01");
        rvz[0x24..0x2C].copy_from_slice(&(u64::MAX / 2).to_be_bytes());
        let input = write(d.path(), "huge.rvz", &rvz);
        let never = d.path().join("never-runs.exe");
        let r = convert(&DOLPHIN, &never, &input, &d.path().join("tmp"));
        if free_space(d.path()).is_some() {
            assert_eq!(r.err(), Some(ToolError::NoSpace(u64::MAX / 2 + SPACE_MARGIN)));
        }
    }
}
