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
import { applyFormatOptions, normalizeDetectedFormat } from "./options.js";

// The wasm-pack output is expected at ../pkg/ relative to the package root.
// Consumers must ensure the WASM binary is available (bundled or fetched).
type WasmModule = typeof import("../pkg/snapper_fmt.js");

let wasmModule: WasmModule | null = null;

async function loadWasm(
  module?: BufferSource | WebAssembly.Module,
): Promise<WasmModule> {
  if (wasmModule) return wasmModule;
  // Dynamic import of the wasm-pack generated JS glue.
  // The path is resolved at bundle time by the consuming build tool.
  wasmModule = await import("../pkg/snapper_fmt.js");
  // wasm-pack --target web requires calling the default init function.
  if ("default" in wasmModule && typeof wasmModule.default === "function") {
    if (module === undefined) {
      await (wasmModule as any).default();
    } else {
      await (wasmModule as any).default({ module_or_path: module });
    }
  }
  return wasmModule;
}

/** High-level formatter wrapping the WASM module. */
export class SnapperFormatter {
  private wasm: WasmModule | null = null;

  /** Load and initialize the WASM module. Must be called before formatting. */
  async init(module?: BufferSource | WebAssembly.Module): Promise<void> {
    this.wasm = await loadWasm(module);
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

  /**
   * Detect document format from a filename.
   * `.asc` is OpenPGP armor and returns null, matching the CLI.
   */
  detectFormat(filename: string): Format | null {
    const wasm = this.ensureInit();
    return normalizeDetectedFormat(
      wasm.detectFormat(filename) as unknown as number | null | undefined,
    );
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
    applyFormatOptions(config, options);
  }
}
