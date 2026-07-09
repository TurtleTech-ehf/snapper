//! pandoc-colink: absorb static archive build product into the snapper binary.
//! See native/snapper-pandoc/build-static.sh (ghc -staticlib).
//!
//! Link flags are target-OS specific (Linux / Darwin / Windows). Default cargo
//! builds without this feature never touch GHC.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=native/snapper-pandoc");
    println!("cargo:rerun-if-changed=native/snapper-pandoc/build-static.sh");
    println!("cargo:rerun-if-env-changed=SNAPPER_PANDOC_LIB_DIR");
    println!("cargo:rerun-if-env-changed=SNAPPER_PANDOC_STATIC_LIB");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PANDOC_COLINK");

    if env::var("CARGO_FEATURE_PANDOC_COLINK").is_err() {
        return;
    }

    let archive = find_static_archive().unwrap_or_else(|| {
        panic!(
            "pandoc-colink: missing libsnapper_pandoc.a (static build product).\n\
             From native/snapper-pandoc run: ./build-static.sh\n\
             Or set SNAPPER_PANDOC_STATIC_LIB / SNAPPER_PANDOC_LIB_DIR.\n\
             Colink is a build/absorb path — not shared lib + rpath.\n\
             See native/snapper-pandoc/README.md for Linux / macOS / Windows notes."
        );
    });
    let archive = archive.canonicalize().unwrap_or(archive);
    // MinGW ld rejects the Windows extended-length prefix (`\\?\D:\...`) with
    // "member … in archive is not an object" (GHA run 28965683301). Strip it.
    // (Avoid raw-string edge cases ending in `\`; use normal escapes.)
    let archive = {
        let s = archive.to_string_lossy();
        let stripped = s
            .strip_prefix("\\\\?\\UNC\\")
            .or_else(|| s.strip_prefix("\\\\?\\"))
            .or_else(|| s.strip_prefix("//?/UNC/"))
            .or_else(|| s.strip_prefix("//?/"))
            .unwrap_or(s.as_ref());
        PathBuf::from(stripped)
    };
    let target = env::var("TARGET").unwrap_or_else(|_| env::var("HOST").unwrap_or_default());

    if target.contains("apple-darwin") || target.contains("apple-ios") {
        emit_darwin(&archive);
    } else if target.contains("windows") {
        emit_windows(&archive);
    } else {
        // Linux and other Unix-likes (proven baseline).
        emit_linux(&archive);
    }

    println!("cargo:rustc-cfg=pandoc_colink");
    println!(
        "cargo:warning=pandoc-colink: static absorb {} for target {target}",
        archive.display()
    );
}

/// Emit a link arg for final artifacts that need the HS archive.
///
/// - Always: bins (`rustc-link-arg-bins`) — the colink CLI product.
/// - Windows only: also cdylibs. crate-type includes cdylib; with
///   pandoc-colink the rlib/cdylib object code references snapper_pandoc_*
///   (GHA 28988119037 left those four undefs when cdylib had no archive).
///   Use the same start-group args as bins — never --whole-archive (that
///   produced ~190k ghc-prim undefs, GHA 28986234307). Linux/mac cdylibs
///   stay archive-free (not PIC-safe — GHA 28963791512).
fn link_arg(arg: &str) {
    println!("cargo:rustc-link-arg-bins={arg}");
    let is_windows = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    if is_windows {
        println!("cargo:rustc-cdylib-link-arg={arg}");
    }
}

fn emit_linux(archive: &Path) {
    // Never put -no-pie in global RUSTFLAGS — that breaks proc-macro .so links.
    // Bins only: -no-pie (cdylibs are shared; -no-pie would break them).
    println!("cargo:rustc-link-arg-bins=-no-pie");
    link_arg("-fuse-ld=bfd");
    link_arg("-Wl,--gc-sections");
    link_arg("-Wl,--no-as-needed");
    link_arg("-Wl,--start-group");
    link_arg(&archive.display().to_string());
    for lib in [
        "m", "z", "gmp", "ffi", "bz2", "lzma", "zstd", "elf", "dw", "numa", "pthread", "dl", "rt",
        "c",
    ] {
        link_arg(&format!("-l{lib}"));
    }
    link_arg("-Wl,--end-group");
    link_arg("-L/usr/lib");
    link_arg("-L/usr/lib64");
}

fn emit_darwin(archive: &Path) {
    // ld64: dead_strip ≈ gc-sections; no --start-group / -no-pie.
    link_arg("-Wl,-dead_strip");
    // Force-load so C exports from the staticlib are not dropped early.
    link_arg(&format!("-Wl,-force_load,{}", archive.display()));
    for lib in ["m", "z", "iconv", "System", "pthread", "dl", "c"] {
        link_arg(&format!("-l{lib}"));
    }
    // GHC RTS / splitmix on Darwin needs Security.framework (SecRandomCopyBytes).
    // Also pull common frameworks that HS staticlibs reference on recent GHC.
    for fw in ["Security", "CoreFoundation", "SystemConfiguration"] {
        link_arg("-framework");
        link_arg(fw);
    }
    // Homebrew / ghcup often put deps here.
    for dir in [
        "/opt/homebrew/lib",
        "/usr/local/lib",
        "/opt/local/lib",
        "/opt/homebrew/opt/gmp/lib",
        "/opt/homebrew/opt/libffi/lib",
    ] {
        if Path::new(dir).is_dir() {
            link_arg(&format!("-L{dir}"));
        }
    }
    // Common C deps pulled by GHC/pandoc staticlibs on macOS when present.
    for lib in ["gmp", "ffi"] {
        link_arg(&format!("-l{lib}"));
    }
}

/// Git-Bash `/c/foo` → `C:/foo` so rustc/`is_dir` and MinGW ld see real Windows paths.
fn win_mingw_path(p: &str) -> PathBuf {
    let p = p.trim();
    if p.len() >= 3 {
        let bytes = p.as_bytes();
        if bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b'/' {
            let drive = (bytes[1] as char).to_ascii_uppercase();
            return PathBuf::from(format!("{drive}:/{}", &p[3..]));
        }
    }
    PathBuf::from(p)
}

/// Split SNAPPER_MINGW_LIB. Only `;` / newlines — never `:` (that chops `C:/…` drive letters).
fn split_mingw_lib_dirs(extra: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for part in extra.split([';', '\n', '\r']) {
        let part = part.trim();
        if !part.is_empty() {
            out.push(win_mingw_path(part));
        }
    }
    out
}

fn find_named_lib(dirs: &[PathBuf], names: &[&str]) -> Option<PathBuf> {
    for d in dirs {
        for name in names {
            let p = d.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn emit_windows(archive: &Path) {
    // Prefer linking the archive path directly. MinGW-built .a may require
    // x86_64-pc-windows-gnu; MSVC often cannot consume mingw .a — build fails loudly.
    let s = archive.display().to_string().replace('\\', "/");
    let is_gnu = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu");
    let is_msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");

    // Search paths for libgmp / libffi (often in GHC bindist mingw/lib, not plain MinGW).
    println!("cargo:rerun-if-env-changed=SNAPPER_MINGW_BIN");
    println!("cargo:rerun-if-env-changed=SNAPPER_GHC_REAL");
    println!("cargo:rerun-if-env-changed=SNAPPER_MINGW_LIB");
    println!("cargo:rerun-if-env-changed=SNAPPER_MINGW_GMP");
    println!("cargo:rerun-if-env-changed=SNAPPER_MINGW_FFI");
    let mut lib_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(extra) = env::var("SNAPPER_MINGW_LIB") {
        // GHA run 28984590129: split on ':' turned C:/msys64/... into "C" + "/msys64/..."
        // so -lgmp was never found despite libgmp.a on disk.
        lib_dirs.extend(split_mingw_lib_dirs(&extra));
    }
    if let Ok(bin) = env::var("SNAPPER_MINGW_BIN") {
        let bin = win_mingw_path(&bin);
        lib_dirs.push(bin.join("..").join("lib"));
        lib_dirs.push(bin.join("..").join("x86_64-w64-mingw32").join("lib"));
    }
    if let Ok(ghc) = env::var("SNAPPER_GHC_REAL") {
        // …/ghc/9.8.4/bin/ghc.exe → …/ghc/9.8.4/mingw/lib
        let ghc = win_mingw_path(&ghc);
        if let Some(root) = ghc.parent().and_then(|p| p.parent()) {
            lib_dirs.push(root.join("mingw").join("lib"));
            lib_dirs.push(root.join("mingw").join("x86_64-w64-mingw32").join("lib"));
            lib_dirs.push(root.join("lib"));
        }
    }
    for d in [
        r"C:\msys64\mingw64\lib",
        r"C:\mingw64\lib",
        r"C:\ProgramData\mingw64\mingw64\lib",
        r"C:\tools\msys64\mingw64\lib",
        "/c/msys64/mingw64/lib",
        "/c/mingw64/lib",
        "/mingw64/lib",
    ] {
        lib_dirs.push(win_mingw_path(d));
    }

    // Prefer absolute paths to libgmp.a / libffi.a (avoids -L search bugs).
    let gmp_abs = env::var("SNAPPER_MINGW_GMP")
        .ok()
        .map(|p| win_mingw_path(&p))
        .filter(|p| p.is_file())
        .or_else(|| find_named_lib(&lib_dirs, &["libgmp.a", "gmp.lib"]));
    let ffi_abs = env::var("SNAPPER_MINGW_FFI")
        .ok()
        .map(|p| win_mingw_path(&p))
        .filter(|p| p.is_file())
        .or_else(|| find_named_lib(&lib_dirs, &["libffi.a", "ffi.lib"]));

    for d in &lib_dirs {
        if d.is_dir() {
            // Forward slashes: backslashes in -L… confuse GNU ld when passed via rustc.
            let p = d.display().to_string().replace('\\', "/");
            link_arg(&format!("-L{p}"));
        }
    }
    if let Some(ref g) = gmp_abs {
        println!(
            "cargo:warning=pandoc-colink: gmp archive {}",
            g.display().to_string().replace('\\', "/")
        );
    }
    if let Some(ref f) = ffi_abs {
        println!(
            "cargo:warning=pandoc-colink: ffi archive {}",
            f.display().to_string().replace('\\', "/")
        );
    }

    if s.ends_with(".lib") {
        link_arg(&s);
    } else if is_gnu {
        // MinGW/GNU ld: Linux-style start-group (no --whole-archive, no
        // --gc-sections). whole-archive + PE gc-sections pulled every HS
        // object then dropped defining sections / left 190k undefs (2d9111c).
        // start-group resolves the C FFI entry → transitive archive members.
        link_arg("-Wl,--allow-multiple-definition");
        link_arg("-Wl,--start-group");
        link_arg(&s);
        if let Some(g) = gmp_abs {
            link_arg(&g.display().to_string().replace('\\', "/"));
        } else {
            link_arg("-lgmp");
        }
        if let Some(f) = ffi_abs {
            link_arg(&f.display().to_string().replace('\\', "/"));
        } else {
            link_arg("-lffi");
        }
        for lib in ["z", "ws2_32", "user32", "shell32", "advapi32", "kernel32", "pthread"] {
            link_arg(&format!("-l{lib}"));
        }
        link_arg("-Wl,--end-group");
    } else {
        link_arg(&s);
        link_arg("-Wl,--allow-multiple-definition");
    }
    if is_msvc {
        println!(
            "cargo:warning=pandoc-colink on windows-msvc: ensure the static archive \
             was built for MSVC or use windows-gnu; mingw .a often fails to link"
        );
    }
}

fn find_static_archive() -> Option<PathBuf> {
    if let Ok(p) = env::var("SNAPPER_PANDOC_STATIC_LIB") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(d) = env::var("SNAPPER_PANDOC_LIB_DIR") {
        let dir = PathBuf::from(&d);
        for name in ["libsnapper_pandoc.a", "snapper_pandoc.lib", "libsnapper_pandoc.lib"] {
            let a = dir.join(name);
            if a.is_file() {
                return Some(a);
            }
        }
    }
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    for c in [
        manifest.join("native/snapper-pandoc/lib/libsnapper_pandoc.a"),
        manifest.join("native/snapper-pandoc/lib/snapper_pandoc.lib"),
        manifest.join("native/snapper-pandoc/static-build/libsnapper_pandoc.a"),
    ] {
        if c.is_file() {
            return Some(c);
        }
    }
    let dist = manifest.join("native/snapper-pandoc/dist-newstyle");
    if dist.is_dir() {
        if let Some(found) = find_file(&dist, "libsnapper_pandoc.a") {
            return Some(found);
        }
    }
    None
}

fn find_file(root: &Path, name: &str) -> Option<PathBuf> {
    fn walk(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
        if depth > 10 {
            return None;
        }
        for ent in std::fs::read_dir(dir).ok()?.flatten() {
            let p = ent.path();
            if p.is_file() && p.file_name().and_then(|s| s.to_str()) == Some(name) {
                return Some(p);
            }
            if p.is_dir() {
                if let Some(f) = walk(&p, name, depth + 1) {
                    return Some(f);
                }
            }
        }
        None
    }
    walk(root, name, 0)
}
