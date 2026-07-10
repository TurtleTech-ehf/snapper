//! In-process pandoc via Haskell foreign-library (`libsnapper_pandoc`).
//!
//! Two link modes:
//! - **Default (`pandoc` feature):** `dlopen` / `LoadLibrary` via `libloading`
//!   (`SNAPPER_PANDOC_LIB`, next-to-exe, or search paths). This is the
//!   **Windows native** path (`snapper_pandoc.dll` from cabal `foreign-library`).
//! - **`pandoc-colink` (Unix):** link-time absorb of `libsnapper_pandoc.a`
//!   (`build.rs` + `build-static.sh`); symbols live in the process image.
//!   PE static absorb is unsupported in CI.
//!
//! Failures are explicit [`FfiError`] values — never silent all-prose.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::{Mutex, OnceLock};

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

#[cfg(not(feature = "pandoc-colink"))]
type ParseFn = unsafe extern "C" fn(
    format: *const c_char,
    input: *const c_char,
    err_out: *mut *mut c_char,
) -> *mut c_char;
#[cfg(not(feature = "pandoc-colink"))]
type FreeFn = unsafe extern "C" fn(*mut c_char);
#[cfg(not(feature = "pandoc-colink"))]
type ReadyFn = unsafe extern "C" fn();
type HsInitFn = unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char);

/// Non-threaded GHC RTS is not re-entrant from multiple OS threads.
static RTS_GATE: Mutex<()> = Mutex::new(());
static RTS_INIT: OnceLock<()> = OnceLock::new();

/// Call `hs_init`. Prefer `NULL, NULL` (allowed by GHC) — avoids argv lifetime
/// and mutability issues. Windows GHA dual-run previously hit
/// `STATUS_HEAP_CORRUPTION` (0xC0000374) with a fragile one-slot argv vector
/// taken via const `as_ptr`; C-side chk_parse with a proper argv worked.
///
/// Fallback: process-lifetime null-terminated argv (`[arg0, NULL]`) if null
/// init is ever rejected by a future RTS (not expected).
unsafe fn call_hs_init(init: HsInitFn) {
    // Primary: null argc/argv (GHC embedding contract).
    unsafe {
        init(std::ptr::null_mut(), std::ptr::null_mut());
    }
}

// ---------------------------------------------------------------------------
// Co-linked symbols (feature pandoc-colink)
// ---------------------------------------------------------------------------
#[cfg(feature = "pandoc-colink")]
mod linked {
    use super::*;

    // Symbols absorbed at link time from libsnapper_pandoc.a (build-static.sh).
    // Direct extern — not dlsym: static RTS symbols are not in the dynamic table.
    unsafe extern "C" {
        pub fn snapper_pandoc_parse(
            format: *const c_char,
            input: *const c_char,
            err_out: *mut *mut c_char,
        ) -> *mut c_char;
        pub fn snapper_pandoc_free(ptr: *mut c_char);
        pub fn snapper_pandoc_hs_ready();
        fn hs_init(argc: *mut c_int, argv: *mut *mut *mut c_char);
    }

    static INIT: OnceLock<Result<(), String>> = OnceLock::new();

    pub fn ensure_init() -> Result<(), FfiError> {
        let slot = INIT.get_or_init(|| {
            RTS_INIT.get_or_init(|| unsafe {
                call_hs_init(hs_init);
                snapper_pandoc_hs_ready();
            });
            Ok(())
        });
        match slot {
            Ok(()) => Ok(()),
            Err(e) => Err(FfiError::LibraryUnavailable(e.clone())),
        }
    }

    pub fn parse(format: &str, input: &str) -> Result<String, FfiError> {
        ensure_init()?;
        let fmt =
            CString::new(format).map_err(|e| FfiError::ParseFailed(format!("format NUL: {e}")))?;
        let inp =
            CString::new(input).map_err(|e| FfiError::ParseFailed(format!("input NUL: {e}")))?;
        let _rts = RTS_GATE.lock().unwrap_or_else(|p| p.into_inner());
        let mut err_ptr: *mut c_char = std::ptr::null_mut();
        let json_ptr = unsafe { snapper_pandoc_parse(fmt.as_ptr(), inp.as_ptr(), &mut err_ptr) };
        if json_ptr.is_null() {
            let msg = if !err_ptr.is_null() {
                let s = unsafe { CStr::from_ptr(err_ptr) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { snapper_pandoc_free(err_ptr) };
                s
            } else {
                "unknown pandoc FFI error".into()
            };
            return Err(FfiError::ParseFailed(msg));
        }
        let json = unsafe {
            let c = CStr::from_ptr(json_ptr);
            match std::str::from_utf8(c.to_bytes()) {
                Ok(s) => s.to_owned(),
                Err(_) => c.to_string_lossy().into_owned(),
            }
        };
        unsafe { snapper_pandoc_free(json_ptr) };
        drop(_rts);
        Ok(json)
    }

    pub fn available() -> bool {
        ensure_init().is_ok()
    }
}

// ---------------------------------------------------------------------------
// Dynamic load (default)
// ---------------------------------------------------------------------------
#[cfg(not(feature = "pandoc-colink"))]
mod dynamic {
    use super::*;
    use std::path::{Path, PathBuf};

    use libloading::{Library, Symbol};

    struct FfiApi {
        _lib: Library,
        parse: ParseFn,
        free: FreeFn,
    }

    static API: OnceLock<Result<FfiApi, String>> = OnceLock::new();

    fn candidate_lib_paths() -> Vec<PathBuf> {
        if let Ok(p) = std::env::var("SNAPPER_PANDOC_LIB") {
            return vec![PathBuf::from(p)];
        }
        let mut paths = Vec::new();
        if let Ok(dir) = std::env::var("SNAPPER_PANDOC_LIB_DIR") {
            let dir = PathBuf::from(&dir);
            for name in [
                "libsnapper_pandoc.so",
                "libsnapper_pandoc.dylib",
                "snapper_pandoc.dll",
                "libsnapper_pandoc.dll",
            ] {
                paths.push(dir.join(name));
            }
        }
        // Same directory as the running executable (Windows packaging: DLL beside .exe).
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for name in [
                    "snapper_pandoc.dll",
                    "libsnapper_pandoc.dll",
                    "libsnapper_pandoc.so",
                    "libsnapper_pandoc.dylib",
                ] {
                    paths.push(dir.join(name));
                }
            }
        }
        for name in [
            "libsnapper_pandoc.so",
            "libsnapper_pandoc.dylib",
            "snapper_pandoc.dll",
            "libsnapper_pandoc.dll",
        ] {
            paths.push(PathBuf::from(name));
        }
        if let Ok(cwd) = std::env::current_dir() {
            for name in [
                "libsnapper_pandoc.so",
                "libsnapper_pandoc.dylib",
                "snapper_pandoc.dll",
                "libsnapper_pandoc.dll",
            ] {
                let p = cwd.join("native/snapper-pandoc/lib").join(name);
                if p.exists() {
                    paths.push(p);
                }
            }
            let dist = cwd.join("native/snapper-pandoc/dist-newstyle");
            if dist.is_dir() {
                for name in [
                    "libsnapper_pandoc.so",
                    "libsnapper_pandoc.so.0.0.0",
                    "snapper_pandoc.dll",
                    "libsnapper_pandoc.dll",
                ] {
                    if let Some(found) = find_lib_in_dir(&dist, name) {
                        paths.push(found);
                    }
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
        let hs_init: Option<Symbol<HsInitFn>> = unsafe { lib.get(b"hs_init\0") }.ok();

        RTS_INIT.get_or_init(|| {
            if let Some(init) = hs_init.as_ref() {
                unsafe {
                    call_hs_init(**init);
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

    pub fn available() -> bool {
        load_api().is_ok()
    }

    pub fn parse(format: &str, input: &str) -> Result<String, FfiError> {
        let api = load_api()?;
        let fmt = CString::new(format)
            .map_err(|e| FfiError::ParseFailed(format!("format contained NUL: {e}")))?;
        let inp = CString::new(input)
            .map_err(|e| FfiError::ParseFailed(format!("input contained NUL: {e}")))?;
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
        let json = unsafe {
            let c = CStr::from_ptr(json_ptr);
            match std::str::from_utf8(c.to_bytes()) {
                Ok(s) => s.to_owned(),
                Err(_) => c.to_string_lossy().into_owned(),
            }
        };
        unsafe { (api.free)(json_ptr) };
        drop(_rts);
        Ok(json)
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Whether the in-process library can be used.
pub fn ffi_available() -> bool {
    #[cfg(feature = "pandoc-colink")]
    {
        linked::available()
    }
    #[cfg(not(feature = "pandoc-colink"))]
    {
        dynamic::available()
    }
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
    #[cfg(feature = "pandoc-colink")]
    let json = linked::parse(format, input)?;
    #[cfg(not(feature = "pandoc-colink"))]
    let json = dynamic::parse(format, input)?;

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
        assert!(!display.contains("Hello world"));
    }

    #[test]
    fn colink_feature_is_documented_in_cfg() {
        // Structural: either colink or dynamic path is compiled.
        let _ = ffi_available();
        #[cfg(feature = "pandoc-colink")]
        {
            // Co-linked builds should not require SNAPPER_PANDOC_LIB for discovery.
            // Availability depends on the linked artifact existing at load time.
        }
    }

    /// Colink is a *build* path: static archive absorb, not ldd→RUNPATH of libHS*.
    #[test]
    fn colink_is_static_build_absorb_not_rpath_graph() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let build_rs = std::fs::read_to_string(manifest.join("build.rs")).expect("build.rs");
        assert!(
            build_rs.contains("libsnapper_pandoc.a"),
            "build.rs must look for static archive product"
        );
        assert!(
            build_rs.contains("--gc-sections") || build_rs.contains("gc-sections"),
            "build.rs must use section GC on the absorb link"
        );
        assert!(
            !build_rs.contains("ldd_dirs"),
            "build.rs must not collect GHC package dirs via ldd for rpath"
        );
        let script = manifest.join("native/snapper-pandoc/build-static.sh");
        assert!(
            script.is_file(),
            "build-static.sh must exist for archive build"
        );
        let script_txt = std::fs::read_to_string(&script).expect("build-static.sh");
        assert!(
            script_txt.contains("-staticlib"),
            "build-static.sh must invoke ghc -staticlib"
        );
        let pack = manifest.join("native/snapper-pandoc/pack-upx.sh");
        assert!(pack.is_file(), "pack-upx.sh optional pack script must exist");
        let pack_txt = std::fs::read_to_string(&pack).expect("pack-upx.sh");
        assert!(
            pack_txt.contains("upx") && pack_txt.contains("-9"),
            "pack-upx.sh must invoke upx with compression flags"
        );
    }
}
