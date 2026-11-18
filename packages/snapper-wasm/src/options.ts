import { Format, type FormatOptions } from "./types.js";

/**
 * Methods the published wrapper forwards onto the wasm-bindgen config.
 *
 * `set_format` takes `any` because the generated enum is not the `Format`
 * enum in this package. The call site already casts.
 */
export interface WasmConfigLike {
  set_format: (format: any) => void;
  set_max_width: (width: number) => void;
  set_lang: (lang: string) => void;
  add_abbreviation: (abbrev: string) => void;
  set_path: (path: string) => void;
  add_ignore: (pattern: string) => void;
}

/** Copy {@link FormatOptions} onto a wasm config, including path and ignore. */
export function applyFormatOptions(
  config: WasmConfigLike,
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
  if (options.path !== undefined) {
    config.set_path(options.path);
  }
  if (options.ignore) {
    for (const pattern of options.ignore) {
      config.add_ignore(pattern);
    }
  }
}

/**
 * `detect_format` returns `undefined` for an extension the CLI refuses.
 * The published wrapper surfaces that as `null` instead of a `Format`.
 */
export function normalizeDetectedFormat(
  value: number | null | undefined,
): Format | null {
  if (value === undefined || value === null) return null;
  return value as Format;
}
