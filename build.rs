//! Optional co-link of the Haskell foreign-library `libsnapper_pandoc`.
//!
//! Enabled by feature `pandoc-colink`. Locates the shared library (from env or
//! `native/snapper-pandoc/lib`), then emits link-search + rpath so the snapper
//! binary is built against it at link time instead of only dlopen at runtime.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=native/snapper-pandoc");
    println!("cargo:rerun-if-env-changed=SNAPPER_PANDOC_LIB");
    println!("cargo:rerun-if-env-changed=SNAPPER_PANDOC_LIB_DIR");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PANDOC_COLINK");

    if env::var("CARGO_FEATURE_PANDOC_COLINK").is_err() {
        return;
    }

    let libdir = find_libdir().unwrap_or_else(|| {
        panic!(
            "pandoc-colink: cannot find libsnapper_pandoc.\n\
             Build native/snapper-pandoc (cabal build snapper_pandoc) and set\n\
             SNAPPER_PANDOC_LIB_DIR to the directory containing libsnapper_pandoc.so,\n\
             or place it at native/snapper-pandoc/lib/."
        );
    });

    let libdir = libdir
        .canonicalize()
        .unwrap_or_else(|_| libdir.clone());
    let libdir_s = libdir.display().to_string();

    println!("cargo:rustc-link-search=native={libdir_s}");
    let so = find_so_file(&libdir).unwrap_or_else(|| {
        panic!("pandoc-colink: no libsnapper_pandoc.so in {}", libdir_s);
    });
    let so = so.canonicalize().unwrap_or(so);
    // Put the shared object early and late so both mold and bfd resolve symbols.
    println!("cargo:rustc-link-arg=-Wl,--no-as-needed");
    println!("cargo:rustc-link-arg={}", so.display());
    println!("cargo:rustc-link-lib=dylib=snapper_pandoc");
    println!("cargo:rustc-link-arg=-Wl,--as-needed");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{libdir_s}");
    println!("cargo:rustc-cfg=pandoc_colink");

    // Transitive rpath for GHC/pandoc deps (from ldd). Never inject
    // libc/libm/ld-linux dirs: an older glibc on rpath steals resolution from
    // the process and breaks host GLIBC_x.y symbol versions.
    for dir in ldd_dirs(&so) {
        if dir == libdir || is_system_lib_dir(&dir) {
            continue;
        }
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
    }

    println!(
        "cargo:warning=pandoc-colink: linking {} into snapper",
        so.display()
    );
}

/// Paths that must stay on the default dynamic-loader search, not rpath.
fn is_system_lib_dir(dir: &Path) -> bool {
    let s = dir.to_string_lossy();
    if s == "/lib" || s == "/lib64" || s == "/usr/lib" || s == "/usr/lib64" {
        return true;
    }
    if s.starts_with("/lib/") || s.starts_with("/lib64/") {
        return true;
    }
    if s.starts_with("/usr/lib/") || s.starts_with("/usr/lib64/") {
        return true;
    }
    if s.contains("ld-linux") {
        return true;
    }
    // Nix store paths look like `/nix/store/<hash>-glibc-2.42-67/lib` — match
    // package stems with `-name-`, not `/name-` (the hash sits before the name).
    // Keep GHC package dirs (`...-pandoc-.../lib/ghc-...`) via the allow below.
    if s.contains("/ghc-") || s.contains("/x86_64-linux-ghc-") {
        return false;
    }
    for marker in [
        "-glibc-",
        "/glibc/",
        "-gcc-",
        "-libgcc",
        "-zlib-",
        "-zstd-",
        "-xz-",
        "-bzip2-",
        "-gmp-",
        "-gmp-with-cxx-",
        "-libffi-",
        "-ncurses-",
        "-openssl-",
        "-elfutils-",
        "-numactl-",
        "-attr-",
        "-acl-",
    ] {
        if s.contains(marker) {
            return true;
        }
    }
    false
}

fn find_libdir() -> Option<PathBuf> {
    if let Ok(p) = env::var("SNAPPER_PANDOC_LIB") {
        let path = PathBuf::from(p);
        if let Some(parent) = path.parent() {
            return Some(parent.to_path_buf());
        }
    }
    if let Ok(d) = env::var("SNAPPER_PANDOC_LIB_DIR") {
        let p = PathBuf::from(d);
        if p.join("libsnapper_pandoc.so").exists()
            || p.join("libsnapper_pandoc.so.0.0.0").exists()
        {
            return Some(p);
        }
    }
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let candidates = [
        manifest.join("native/snapper-pandoc/lib"),
        manifest.join("native/snapper-pandoc/dist-newstyle"),
    ];
    for c in &candidates {
        if c.join("libsnapper_pandoc.so").exists()
            || c.join("libsnapper_pandoc.so.0.0.0").exists()
        {
            return Some(c.clone());
        }
        if c.is_dir() {
            if let Some(found) = find_file(c, "libsnapper_pandoc.so") {
                return found.parent().map(|p| p.to_path_buf());
            }
            if let Some(found) = find_file(c, "libsnapper_pandoc.so.0.0.0") {
                return found.parent().map(|p| p.to_path_buf());
            }
        }
    }
    None
}

fn find_so_file(libdir: &Path) -> Option<PathBuf> {
    let a = libdir.join("libsnapper_pandoc.so");
    if a.exists() {
        return Some(a);
    }
    let b = libdir.join("libsnapper_pandoc.so.0.0.0");
    if b.exists() {
        return Some(b);
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

fn ldd_dirs(so: &Path) -> Vec<PathBuf> {
    let out = Command::new("ldd").arg(so).output().ok();
    let Some(out) = out else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut dirs = Vec::new();
    for line in text.lines() {
        // "libfoo.so => /path/to/libfoo.so (0x...)"
        if let Some(idx) = line.find("=>") {
            let rest = line[idx + 2..].trim();
            let path = rest.split_whitespace().next().unwrap_or("");
            if path.starts_with('/') {
                if let Some(parent) = Path::new(path).parent() {
                    let p = parent.to_path_buf();
                    if !dirs.contains(&p) {
                        dirs.push(p);
                    }
                }
            }
        }
    }
    dirs
}
