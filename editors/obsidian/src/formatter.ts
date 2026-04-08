import { Editor } from "obsidian";
import { SnapperFormatter, Format, type FormatOptions } from "@snapper/wasm";
import type { SnapperSettings } from "./settings";

let formatter: SnapperFormatter | null = null;

export async function initFormatter(): Promise<void> {
  formatter = new SnapperFormatter();
  await formatter.init();
}

function buildOptions(settings: SnapperSettings, filename?: string): FormatOptions {
  const opts: FormatOptions = {
    maxWidth: settings.maxWidth,
    lang: settings.lang,
  };

  if (settings.extraAbbreviations.trim()) {
    opts.extraAbbreviations = settings.extraAbbreviations
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
  }

  if (filename && formatter) {
    opts.format = formatter.detectFormat(filename);
  } else {
    opts.format = Format.Markdown;
  }

  return opts;
}

export function formatEditor(
  editor: Editor,
  settings: SnapperSettings,
  filename?: string,
): void {
  if (!formatter) return;

  const cursor = editor.getCursor();
  const input = editor.getValue();
  const opts = buildOptions(settings, filename);

  const output = formatter.formatText(input, opts);
  if (output !== input) {
    editor.setValue(output);
    editor.setCursor(cursor);
  }
}

export function formatSelection(
  editor: Editor,
  settings: SnapperSettings,
  filename?: string,
): void {
  if (!formatter) return;

  const selection = editor.getSelection();
  if (!selection) return;

  const opts = buildOptions(settings, filename);
  const output = formatter.formatText(selection, opts);

  if (output !== selection) {
    editor.replaceSelection(output);
  }
}
