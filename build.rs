//! pandoc-colink: absorb static archive build product into the snapper binary.
//! See native/snapper-pandoc/build-static.sh (ghc -staticlib).

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
             Colink is a build/absorb path — not shared lib + rpath."
        );
    });
    let archive = archive.canonicalize().unwrap_or(archive);

    // Bin-only absorb (package also has cdylib).
    bin_arg("-Wl,--gc-sections");
    // HS staticlib objects are not always PIE-safe; non-PIE bin is fine for CLI.
    bin_arg("-no-pie");
    bin_arg("-Wl,--no-as-needed");
    // Group archive with system libs so mutual refs (TLS, zlib, elf) resolve.
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

    println!("cargo:rustc-cfg=pandoc_colink");
    println!(
        "cargo:warning=pandoc-colink: static absorb {} into bins (build product, not rpath graph)",
        archive.display()
    );
}

fn bin_arg(arg: &str) {
    println!("cargo:rustc-link-arg-bins={arg}");
}

fn find_static_archive() -> Option<PathBuf> {
    if let Ok(p) = env::var("SNAPPER_PANDOC_STATIC_LIB") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(d) = env::var("SNAPPER_PANDOC_LIB_DIR") {
        let a = PathBuf::from(&d).join("libsnapper_pandoc.a");
        if a.is_file() {
            return Some(a);
        }
    }
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    for c in [
        manifest.join("native/snapper-pandoc/lib/libsnapper_pandoc.a"),
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
