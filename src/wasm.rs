//! WebAssembly entry point for snapper.
//!
//! Exposes the core formatting API via `wasm_bindgen` for use in
//! browser-based editors (Obsidian, Word add-ins, etc.).

use wasm_bindgen::prelude::*;

use crate::FormatConfig;
use crate::format::Format;

/// Document format (WASM-exported mirror of [`Format`]).
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmFormat {
    Org,
    Latex,
    Markdown,
    Rst,
    Plaintext,
    Typst,
    AsciiDoc,
}

impl From<WasmFormat> for Format {
    fn from(f: WasmFormat) -> Self {
        match f {
            WasmFormat::Org => Format::Org,
            WasmFormat::Latex => Format::Latex,
            WasmFormat::Markdown => Format::Markdown,
            WasmFormat::Rst => Format::Rst,
            WasmFormat::Plaintext => Format::Plaintext,
            WasmFormat::Typst => Format::Typst,
            WasmFormat::AsciiDoc => Format::AsciiDoc,
        }
    }
}

impl From<Format> for WasmFormat {
    fn from(f: Format) -> Self {
        match f {
            Format::Org => WasmFormat::Org,
            Format::Latex => WasmFormat::Latex,
            Format::Markdown => WasmFormat::Markdown,
            Format::Rst => WasmFormat::Rst,
            Format::Plaintext => WasmFormat::Plaintext,
            Format::Typst => WasmFormat::Typst,
            Format::AsciiDoc => WasmFormat::AsciiDoc,
        }
    }
}

/// Formatting configuration for the WASM API.
#[wasm_bindgen]
pub struct WasmConfig {
    format: WasmFormat,
    max_width: usize,
    lang: String,
    extra_abbreviations: Vec<String>,
    path: Option<String>,
    ignore: Vec<String>,
}

#[wasm_bindgen]
impl WasmConfig {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            format: WasmFormat::Plaintext,
            max_width: 0,
            lang: "en".to_string(),
            extra_abbreviations: vec![],
            path: None,
            ignore: Vec::new(),
        }
    }

    /// Path the ignore globs are matched against. Uses the CLI `[[path]]` matcher.
    pub fn set_path(&mut self, path: &str) {
        self.path = Some(path.to_string());
    }

    /// Add a glob. A match leaves the text unchanged, same as a CLI ignore rule.
    pub fn add_ignore(&mut self, pattern: &str) {
        self.ignore.push(pattern.to_string());
    }

    /// Set the document format.
    pub fn set_format(&mut self, format: WasmFormat) {
        self.format = format;
    }

    /// Set maximum line width (0 = unlimited).
    pub fn set_max_width(&mut self, width: usize) {
        self.max_width = width;
    }

    /// Set language for abbreviation handling (en, de, fr, is, pl).
    pub fn set_lang(&mut self, lang: &str) {
        self.lang = lang.to_string();
    }

    /// Add a project-specific abbreviation.
    pub fn add_abbreviation(&mut self, abbrev: &str) {
        self.extra_abbreviations.push(abbrev.to_string());
    }
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmConfig {
    fn path_is_ignored(&self) -> bool {
        let Some(path) = self.path.as_deref() else {
            return false;
        };
        self.ignore
            .iter()
            .any(|pattern| crate::config::path_rule_matches(pattern, path))
    }

    fn to_format_config(&self) -> FormatConfig {
        FormatConfig {
            format: self.format.into(),
            max_width: self.max_width,
            use_neural: false,
            neural_lang: self.lang.clone(),
            neural_model_path: None,
            extra_abbreviations: self.extra_abbreviations.clone(),
            use_pandoc: false,
            pandoc_format: None,
            code: std::collections::HashMap::new(),
            format_code: false,
            ..Default::default()
        }
    }
}

/// Format text with semantic line breaks.
#[wasm_bindgen(js_name = "formatText")]
pub fn format_text(input: &str, config: &WasmConfig) -> Result<String, JsError> {
    if config.path_is_ignored() {
        return Ok(input.to_string());
    }
    let fc = config.to_format_config();
    crate::format_text(input, &fc).map_err(|e| JsError::new(&e.to_string()))
}

/// Format only lines within a range (1-indexed, inclusive).
#[wasm_bindgen(js_name = "formatRange")]
pub fn format_range(
    input: &str,
    config: &WasmConfig,
    start: usize,
    end: usize,
) -> Result<String, JsError> {
    if config.path_is_ignored() {
        return Ok(input.to_string());
    }
    let fc = config.to_format_config();
    crate::format_range(input, &fc, start, end).map_err(|e| JsError::new(&e.to_string()))
}

/// Detect document format from a filename.
///
/// Uses [`Format::recognized_from_path`], the same function the CLI uses.
/// `.asc` is OpenPGP armor and returns `None` instead of plaintext.
#[wasm_bindgen(js_name = "detectFormat")]
pub fn detect_format(filename: &str) -> Option<WasmFormat> {
    Format::recognized_from_path(std::path::Path::new(filename)).map(WasmFormat::from)
}

/// Return the snapper version string.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_format_does_not_treat_asc_as_plaintext() {
        assert_eq!(detect_format("note.asc"), None);
        assert_eq!(detect_format("note.adoc"), Some(WasmFormat::AsciiDoc));
        assert_eq!(detect_format("notes.md"), Some(WasmFormat::Markdown));
    }

    #[test]
    fn format_text_skips_an_ignored_path() {
        let mut config = WasmConfig::new();
        config.set_format(WasmFormat::Plaintext);
        config.set_path("skip/a.md");
        config.add_ignore("skip/*.md");
        let input = "Two sentences. Here.\n";
        let out = format_text(input, &config).expect("format");
        assert_eq!(out, input);
    }
}
