import { SnapperFormatter, Format, type FormatOptions } from "@snapper/wasm";

let formatter: SnapperFormatter | null = null;

export async function initFormatter(): Promise<void> {
  formatter = new SnapperFormatter();
  await formatter.init();
}

function defaultOptions(): FormatOptions {
  return { format: Format.Plaintext, maxWidth: 0, lang: "en" };
}

/** Format the entire document body paragraph by paragraph. */
export async function formatDocument(options?: FormatOptions): Promise<void> {
  if (!formatter) return;
  const opts = options ?? defaultOptions();

  await Word.run(async (context) => {
    const paragraphs = context.document.body.paragraphs;
    paragraphs.load("text");
    await context.sync();

    for (const para of paragraphs.items) {
      if (!para.text.trim()) continue;
      const formatted = formatter!.formatText(para.text, opts);
      if (formatted !== para.text) {
        para.insertText(formatted, Word.InsertLocation.replace);
      }
    }
    await context.sync();
  });
}

/** Format the current selection. */
export async function formatSelection(options?: FormatOptions): Promise<void> {
  if (!formatter) return;
  const opts = options ?? defaultOptions();

  await Word.run(async (context) => {
    const selection = context.document.getSelection();
    selection.load("text");
    await context.sync();

    const text = selection.text;
    if (!text.trim()) return;

    const formatted = formatter!.formatText(text, opts);
    if (formatted !== text) {
      selection.insertText(formatted, Word.InsertLocation.replace);
    }
    await context.sync();
  });
}
