/**
 * @snapper/wasm -- WebAssembly wrapper for the snapper formatter.
 *
 * Usage:
 *   const formatter = new SnapperFormatter();
 *   await formatter.init();
 *   const output = formatter.formatText(input, { format: Format.Org });
 */

export { Format, type FormatOptions } from "./types.js";
import { Format, type FormatOptions } from "./types.js";

// The wasm-pack output is expected at ../pkg/ relative to the package root.
// Consumers must ensure the WASM binary is available (bundled or fetched).
type WasmModule = typeof import("../pkg/snapper_fmt.js");

let wasmModule: WasmModule | null = null;

async function loadWasm(): Promise<WasmModule> {
  if (wasmModule) return wasmModule;
  // Dynamic import of the wasm-pack generated JS glue.
  // The path is resolved at bundle time by the consuming build tool.
  wasmModule = await import("../pkg/snapper_fmt.js");
  // wasm-pack --target web requires calling the default init function.
  if ("default" in wasmModule && typeof wasmModule.default === "function") {
    await (wasmModule as any).default();
  }
  return wasmModule;
}

/** High-level formatter wrapping the WASM module. */
export class SnapperFormatter {
  private wasm: WasmModule | null = null;

  /** Load and initialize the WASM module. Must be called before formatting. */
  async init(): Promise<void> {
    this.wasm = await loadWasm();
  }

  private ensureInit(): WasmModule {
    if (!this.wasm) {
      throw new Error("SnapperFormatter not initialized. Call init() first.");
    }
    return this.wasm;
  }

  /** Format text with semantic line breaks. */
  formatText(input: string, options?: FormatOptions): string {
    const wasm = this.ensureInit();
    const config = new wasm.WasmConfig();
    this.applyOptions(config, options);
    try {
      return wasm.formatText(input, config);
    } finally {
      config.free();
    }
  }

  /** Format only lines within a range (1-indexed, inclusive). */
  formatRange(
    input: string,
    start: number,
    end: number,
    options?: FormatOptions,
  ): string {
    const wasm = this.ensureInit();
    const config = new wasm.WasmConfig();
    this.applyOptions(config, options);
    try {
      return wasm.formatRange(input, config, start, end);
    } finally {
      config.free();
    }
  }

  /** Detect document format from a filename. */
  detectFormat(filename: string): Format {
    const wasm = this.ensureInit();
    return wasm.detectFormat(filename) as unknown as Format;
  }

  /** Return the snapper version. */
  version(): string {
    const wasm = this.ensureInit();
    return wasm.version();
  }

  private applyOptions(
    config: InstanceType<WasmModule["WasmConfig"]>,
    options?: FormatOptions,
  ): void {
    if (!options) return;
    if (options.format !== undefined) {
      config.set_format(options.format as unknown as number);
    }
    if (options.maxWidth !== undefined) {
      config.set_max_width(options.maxWidth);
    }
    if (options.lang) {
      config.set_lang(options.lang);
    }
    if (options.extraAbbreviations) {
      for (const abbrev of options.extraAbbreviations) {
        config.add_abbreviation(abbrev);
      }
    }
  }
}
