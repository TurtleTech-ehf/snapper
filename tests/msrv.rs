//! The declared toolchain has to be one darling can compile.
//!
//! rmcp-macros pulls darling 0.23, whose own `rust-version` is 1.88.
//! A crate MSRV of 1.85, or a CI job that never installs 1.88, disagrees
//! with that dependency.

use std::fs;
use std::path::Path;

#[test]
fn msrv_is_1_88_and_ci_agrees() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let rust_version = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("rust-version"))
        .expect("Cargo.toml rust-version");
    assert!(
        rust_version.contains("\"1.88\""),
        "darling 0.23 needs Rust 1.88, got {rust_version}"
    );
    let pixi = fs::read_to_string(root.join("pixi.toml")).unwrap();
    assert!(
        pixi.contains(">=1.88"),
        "pixi.toml must ask for the same MSRV, got the rust line in:\n{pixi}"
    );
    let workflows = root.join(".github/workflows");
    for entry in fs::read_dir(&workflows).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        if !text.contains("rust-toolchain@") {
            continue;
        }
        assert!(
            !text.contains("rust-toolchain@stable"),
            "{} installs stable instead of the 1.88 MSRV",
            path.display()
        );
        assert!(
            text.contains("rust-toolchain@1.88"),
            "{} must install Rust 1.88 so CI matches Cargo.toml and darling",
            path.display()
        );
    }
}
