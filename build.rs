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

fn bin_arg(arg: &str) {
    println!("cargo:rustc-link-arg-bins={arg}");
}

fn emit_linux(archive: &Path) {
    // Bins only (rustc-link-arg-bins). Never put -no-pie in global RUSTFLAGS —
    // that breaks proc-macro / cdylib shared objects (undefined main).
    bin_arg("-fuse-ld=bfd");
    bin_arg("-Wl,--gc-sections");
    // HS staticlib objects are not always PIE-safe; non-PIE bin is fine for CLI.
    bin_arg("-no-pie");
    bin_arg("-Wl,--no-as-needed");
    bin_arg("-Wl,--start-group");
    bin_arg(&archive.display().to_string());
    for lib in [
        "m", "z", "gmp", "ffi", "bz2", "lzma", "zstd", "elf", "dw", "numa", "pthread", "dl", "rt",
        "c",
    ] {
        bin_arg(&format!("-l{lib}"));
    }
    bin_arg("-Wl,--end-group");
    bin_arg("-L/usr/lib");
    bin_arg("-L/usr/lib64");
}

fn emit_darwin(archive: &Path) {
    // ld64: dead_strip ≈ gc-sections; no --start-group / -no-pie.
    bin_arg("-Wl,-dead_strip");
    // Force-load so C exports from the staticlib are not dropped early.
    bin_arg(&format!("-Wl,-force_load,{}", archive.display()));
    for lib in ["m", "z", "iconv", "System", "pthread", "dl", "c"] {
        bin_arg(&format!("-l{lib}"));
    }
    // GHC RTS / splitmix on Darwin needs Security.framework (SecRandomCopyBytes).
    // Also pull common frameworks that HS staticlibs reference on recent GHC.
    for fw in ["Security", "CoreFoundation", "SystemConfiguration"] {
        bin_arg("-framework");
        bin_arg(fw);
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
            bin_arg(&format!("-L{dir}"));
        }
    }
    // Common C deps pulled by GHC/pandoc staticlibs on macOS when present.
    for lib in ["gmp", "ffi"] {
        bin_arg(&format!("-l{lib}"));
    }
}

fn emit_windows(archive: &Path) {
    // Prefer linking the archive path directly. MinGW-built .a may require
    // x86_64-pc-windows-gnu; MSVC often cannot consume mingw .a — build fails loudly.
    let s = archive.display().to_string();
    let is_gnu = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu");
    let is_msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");

    if s.ends_with(".lib") {
        bin_arg(&s);
    } else if is_gnu {
        // MinGW/GNU ld: whole-archive so HS C exports survive dead-code elimination.
        bin_arg("-Wl,--gc-sections");
        bin_arg("-Wl,--allow-multiple-definition");
        bin_arg("-Wl,--start-group");
        bin_arg("-Wl,--whole-archive");
        bin_arg(&s);
        bin_arg("-Wl,--no-whole-archive");
        for lib in [
            "gmp", "ffi", "z", "ws2_32", "user32", "shell32", "advapi32", "kernel32", "pthread",
        ] {
            bin_arg(&format!("-l{lib}"));
        }
        bin_arg("-Wl,--end-group");
    } else {
        bin_arg(&s);
        bin_arg("-Wl,--gc-sections");
        bin_arg("-Wl,--allow-multiple-definition");
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
