/** Document format for snapper. */
export enum Format {
  Org = 0,
  Latex = 1,
  Markdown = 2,
  Rst = 3,
  Plaintext = 4,
}

/** Options for formatting text. */
export interface FormatOptions {
  /** Document format. Auto-detected from filename if omitted. */
  format?: Format;
  /** Maximum line width (0 = unlimited). */
  maxWidth?: number;
  /** Language for abbreviation handling (en, de, fr, is, pl). */
  lang?: string;
  /** Extra project-specific abbreviations. */
  extraAbbreviations?: string[];
}
