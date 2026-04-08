//! WebAssembly entry point for snapper.
//!
//! Exposes the core formatting API via `wasm_bindgen` for use in
//! browser-based editors (Obsidian, Word add-ins, etc.).

use wasm_bindgen::prelude::*;

use crate::format::Format;
use crate::FormatConfig;

/// Document format (WASM-exported mirror of [`Format`]).
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmFormat {
    Org,
    Latex,
    Markdown,
    Rst,
    Plaintext,
}

impl From<WasmFormat> for Format {
    fn from(f: WasmFormat) -> Self {
        match f {
            WasmFormat::Org => Format::Org,
            WasmFormat::Latex => Format::Latex,
            WasmFormat::Markdown => Format::Markdown,
            WasmFormat::Rst => Format::Rst,
            WasmFormat::Plaintext => Format::Plaintext,
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
        }
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

impl WasmConfig {
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
        }
    }
}

/// Format text with semantic line breaks.
#[wasm_bindgen(js_name = "formatText")]
pub fn format_text(input: &str, config: &WasmConfig) -> Result<String, JsError> {
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
    let fc = config.to_format_config();
    crate::format_range(input, &fc, start, end).map_err(|e| JsError::new(&e.to_string()))
}

/// Detect document format from a filename's extension.
#[wasm_bindgen(js_name = "detectFormat")]
pub fn detect_format(filename: &str) -> WasmFormat {
    let ext = filename.rsplit('.').next().unwrap_or("");
    Format::from_extension(ext).into()
}

/// Return the snapper version string.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
