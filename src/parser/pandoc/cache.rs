//! Content-addressed cache of pandoc JSON ASTs.
//!
//! Obtain-AST dominates cost. After the first successful parse of a given
//! (format, source) pair, reuse JSON from:
//! 1. Process-local memory (hash → Arc<str>)
//! 2. Disk under `$SNAPPER_PANDOC_CACHE_DIR` or
//!    `$XDG_CACHE_HOME/snapper/pandoc-ast` (disable with `SNAPPER_PANDOC_CACHE=0`)
//!
//! Keys are SHA-256 over `format \\0 input`. Values are raw pandoc JSON bytes.
//! This is the practical speed lever for CLI re-runs and multi-file batches
//! without changing the AST→region model.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use sha2::{Digest, Sha256};

static MEM: OnceLock<Mutex<HashMap<[u8; 32], Arc<str>>>> = OnceLock::new();

fn mem() -> &'static Mutex<HashMap<[u8; 32], Arc<str>>> {
    MEM.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Stable key for (format, input).
pub fn cache_key(format: &str, input: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(format.as_bytes());
    h.update([0u8]);
    h.update(input.as_bytes());
    h.finalize().into()
}

fn cache_enabled() -> bool {
    match std::env::var("SNAPPER_PANDOC_CACHE") {
        Ok(v) => {
            let v = v.to_ascii_lowercase();
            !(v == "0" || v == "off" || v == "false" || v == "no")
        }
        Err(_) => true,
    }
}

fn disk_dir() -> Option<PathBuf> {
    if !cache_enabled() {
        return None;
    }
    if let Ok(p) = std::env::var("SNAPPER_PANDOC_CACHE_DIR") {
        return Some(PathBuf::from(p));
    }
    // Prefer XDG, then home, then temp.
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        return Some(PathBuf::from(xdg).join("snapper/pandoc-ast"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home).join(".cache/snapper/pandoc-ast"));
    }
    Some(std::env::temp_dir().join("snapper-pandoc-ast"))
}

fn disk_path(key: &[u8; 32]) -> Option<PathBuf> {
    let dir = disk_dir()?;
    let name = key.iter().map(|b| format!("{b:02x}")).collect::<String>();
    Some(dir.join(name).with_extension("json"))
}

/// Look up cached pandoc JSON for (format, input).
pub fn get_json(format: &str, input: &str) -> Option<Arc<str>> {
    if !cache_enabled() {
        return None;
    }
    let key = cache_key(format, input);
    if let Ok(guard) = mem().lock() {
        if let Some(v) = guard.get(&key) {
            return Some(Arc::clone(v));
        }
    }
    let path = disk_path(&key)?;
    let bytes = fs::read(&path).ok()?;
    let s = String::from_utf8(bytes).ok()?;
    let arc: Arc<str> = Arc::from(s);
    if let Ok(mut guard) = mem().lock() {
        guard.insert(key, Arc::clone(&arc));
    }
    Some(arc)
}

/// Store pandoc JSON for (format, input) in memory and on disk.
pub fn put_json(format: &str, input: &str, json: &str) {
    if !cache_enabled() {
        return;
    }
    let key = cache_key(format, input);
    let arc: Arc<str> = Arc::from(json);
    if let Ok(mut guard) = mem().lock() {
        guard.insert(key, Arc::clone(&arc));
    }
    if let Some(path) = disk_path(&key) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, arc.as_bytes());
    }
}

/// Clear process-local memory cache (tests / benchmarks).
pub fn clear_memory() {
    if let Ok(mut guard) = mem().lock() {
        guard.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_roundtrip_memory() {
        clear_memory();
        let fmt = "markdown";
        let input = "Hello cache. Second.";
        assert!(get_json(fmt, input).is_none());
        put_json(fmt, input, r#"{"pandoc-api-version":[1,23,1],"meta":{},"blocks":[]}"#);
        let hit = get_json(fmt, input).expect("memory hit");
        assert!(hit.contains("pandoc-api-version"));
        clear_memory();
    }

    #[test]
    fn different_inputs_different_keys() {
        assert_ne!(cache_key("markdown", "a"), cache_key("markdown", "b"));
        assert_ne!(cache_key("org", "a"), cache_key("markdown", "a"));
    }
}
