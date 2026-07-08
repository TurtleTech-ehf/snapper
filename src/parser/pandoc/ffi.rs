//! In-process pandoc via Haskell FFI (libsnapper_pandoc).
//!
//! Loads `libsnapper_pandoc` dynamically, initializes the GHC RTS once, and
//! parses markup to pandoc JSON without spawning a pandoc CLI process.
//! Failures are explicit [`FfiError`] values — never silent all-prose.

use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use libloading::{Library, Symbol};
use thiserror::Error;

use super::ast::regions_from_pandoc_json;
use crate::parser::Region;

/// Errors from the in-process pandoc FFI path.
#[derive(Debug, Error)]
pub enum FfiError {
    #[error("pandoc FFI library unavailable: {0}")]
    LibraryUnavailable(String),
    #[error("pandoc FFI parse failed: {0}")]
    ParseFailed(String),
    #[error("pandoc FFI returned invalid AST: {0}")]
    InvalidAst(String),
}

use std::os::raw::{c_char, c_int};

type ParseFn = unsafe extern "C" fn(
    format: *const c_char,
    input: *const c_char,
    err_out: *mut *mut c_char,
) -> *mut c_char;
type FreeFn = unsafe extern "C" fn(*mut c_char);
type ReadyFn = unsafe extern "C" fn();
type HsInitFn = unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char);

struct FfiApi {
    _lib: Library,
    parse: ParseFn,
    free: FreeFn,
}

static API: OnceLock<Result<FfiApi, String>> = OnceLock::new();
static RTS_INIT: OnceLock<()> = OnceLock::new();
/// Non-threaded GHC RTS is not re-entrant from multiple OS threads (rayon,
/// cargo test). Serialize all entries into the foreign library.
static RTS_GATE: Mutex<()> = Mutex::new(());

fn candidate_lib_paths() -> Vec<PathBuf> {
    // Explicit path wins exclusively (enables hard-fail tests and deploy pinning).
    if let Ok(p) = std::env::var("SNAPPER_PANDOC_LIB") {
        return vec![PathBuf::from(p)];
    }
    let mut paths = Vec::new();
    if let Ok(dir) = std::env::var("SNAPPER_PANDOC_LIB_DIR") {
        paths.push(PathBuf::from(&dir).join("libsnapper_pandoc.so"));
    }
    for name in [
        "libsnapper_pandoc.so",
        "libsnapper_pandoc.dylib",
        "snapper_pandoc.dll",
    ] {
        paths.push(PathBuf::from(name));
    }
    if let Ok(cwd) = std::env::current_dir() {
        let lib_dir = cwd.join("native/snapper-pandoc/lib/libsnapper_pandoc.so");
        if lib_dir.exists() {
            paths.push(lib_dir);
        }
        let dist = cwd.join("native/snapper-pandoc/dist-newstyle");
        if dist.is_dir() {
            if let Some(found) = find_lib_in_dir(&dist, "libsnapper_pandoc.so") {
                paths.push(found);
            }
            // Cabal may only emit the versioned soname.
            if let Some(found) = find_lib_in_dir(&dist, "libsnapper_pandoc.so.0.0.0") {
                paths.push(found);
            }
        }
    }
    paths
}

fn find_lib_in_dir(root: &Path, name: &str) -> Option<PathBuf> {
    fn walk(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
        if depth > 8 {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        for ent in entries.flatten() {
            let p = ent.path();
            if p.is_file() && p.file_name().and_then(|s| s.to_str()) == Some(name) {
                return Some(p);
            }
            if p.is_dir() {
                if let Some(found) = walk(&p, name, depth + 1) {
                    return Some(found);
                }
            }
        }
        None
    }
    walk(root, name, 0)
}

fn open_library(path: &Path) -> Result<FfiApi, String> {
    // RTLD_GLOBAL so GHC RTS symbols (hs_init) from DT_NEEDED deps resolve.
    #[cfg(unix)]
    let lib = {
        use libloading::os::unix::{Library as UnixLibrary, RTLD_GLOBAL, RTLD_NOW};
        let flags = RTLD_NOW | RTLD_GLOBAL;
        unsafe { UnixLibrary::open(Some(path), flags) }
            .map(Library::from)
            .map_err(|e| format!("{}: {e}", path.display()))?
    };
    #[cfg(not(unix))]
    let lib = unsafe { Library::new(path) }.map_err(|e| format!("{}: {e}", path.display()))?;

    let parse: Symbol<ParseFn> = unsafe { lib.get(b"snapper_pandoc_parse\0") }
        .map_err(|e| format!("{}: missing snapper_pandoc_parse: {e}", path.display()))?;
    let free: Symbol<FreeFn> = unsafe { lib.get(b"snapper_pandoc_free\0") }
        .map_err(|e| format!("{}: missing snapper_pandoc_free: {e}", path.display()))?;
    let ready: Option<Symbol<ReadyFn>> = unsafe { lib.get(b"snapper_pandoc_hs_ready\0") }.ok();
    // hs_init lives in libHSrts; with RTLD_GLOBAL it is visible after load.
    let hs_init: Option<Symbol<HsInitFn>> = unsafe { lib.get(b"hs_init\0") }.ok();

    RTS_INIT.get_or_init(|| {
        if let Some(init) = hs_init.as_ref() {
            unsafe {
                let mut argc: c_int = 1;
                // Keep argv storage for the process lifetime (RTS may retain pointers).
                static mut ARG0: *mut c_char = std::ptr::null_mut();
                if ARG0.is_null() {
                    ARG0 = CString::new("snapper")
                        .expect("static argv")
                        .into_raw();
                }
                let mut argv_storage = [ARG0];
                let mut argv = argv_storage.as_mut_ptr();
                init(&mut argc, &mut argv);
            }
        }
        if let Some(r) = ready.as_ref() {
            unsafe {
                r();
            }
        }
    });

    Ok(FfiApi {
        parse: *parse,
        free: *free,
        _lib: lib,
    })
}

fn load_api() -> Result<&'static FfiApi, FfiError> {
    let slot = API.get_or_init(|| {
        let mut last_err = String::from("no candidate library path tried");
        for path in candidate_lib_paths() {
            match open_library(&path) {
                Ok(api) => return Ok(api),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    });
    match slot {
        Ok(api) => Ok(api),
        Err(msg) => Err(FfiError::LibraryUnavailable(msg.clone())),
    }
}

/// Whether the in-process library can be loaded (does not parse).
pub fn ffi_available() -> bool {
    load_api().is_ok()
}

/// Parse markup with the in-process pandoc library and classify via AST node kinds.
pub fn parse_via_ffi(input: &str, format: &str) -> Result<Vec<Region>, FfiError> {
    let (regions, _) = parse_via_ffi_with_json(input, format)?;
    Ok(regions)
}

/// Like [`parse_via_ffi`], also returns the pandoc JSON for caching.
pub fn parse_via_ffi_with_json(
    input: &str,
    format: &str,
) -> Result<(Vec<Region>, String), FfiError> {
    let api = load_api()?;
    let fmt = CString::new(format)
        .map_err(|e| FfiError::ParseFailed(format!("format contained NUL: {e}")))?;
    let inp = CString::new(input)
        .map_err(|e| FfiError::ParseFailed(format!("input contained NUL: {e}")))?;

    // Hold the gate for the whole foreign call + free of returned C strings.
    let _rts = RTS_GATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut err_ptr: *mut c_char = std::ptr::null_mut();
    let json_ptr = unsafe { (api.parse)(fmt.as_ptr(), inp.as_ptr(), &mut err_ptr) };
    if json_ptr.is_null() {
        let msg = if !err_ptr.is_null() {
            let s = unsafe { CStr::from_ptr(err_ptr) }
                .to_string_lossy()
                .into_owned();
            unsafe { (api.free)(err_ptr) };
            s
        } else {
            "unknown pandoc FFI error (null result, no message)".to_string()
        };
        return Err(FfiError::ParseFailed(msg));
    }
    // Copy JSON bytes before free; avoid lossy UTF-8 re-encode when valid.
    let json = unsafe {
        let c = CStr::from_ptr(json_ptr);
        let bytes = c.to_bytes();
        match std::str::from_utf8(bytes) {
            Ok(s) => s.to_owned(),
            Err(_) => c.to_string_lossy().into_owned(),
        }
    };
    unsafe { (api.free)(json_ptr) };
    drop(_rts);
    // Classify outside the RTS gate — pure Rust (hot path after obtain-AST).
    let regions = regions_from_pandoc_json(&json).map_err(FfiError::InvalidAst)?;
    Ok((regions, json))
}

/// Explicit library-unavailable error for tests of the failure contract.
pub fn ffi_library_unavailable_error(detail: impl Into<String>) -> FfiError {
    FfiError::LibraryUnavailable(detail.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_unavailable_is_explicit_error_not_all_prose() {
        let err = ffi_library_unavailable_error("forced unavailable for test");
        match &err {
            FfiError::LibraryUnavailable(msg) => {
                assert!(msg.contains("forced"));
            }
            other => panic!("expected LibraryUnavailable, got {other}"),
        }
        let display = err.to_string();
        assert!(
            display.contains("unavailable"),
            "error must be explicit, got: {display}"
        );
        // Contract: never treat this as success with a single Prose(input) region.
        assert!(!display.contains("Hello world"));
    }
}
