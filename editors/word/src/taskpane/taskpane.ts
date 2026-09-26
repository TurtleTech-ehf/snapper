import { initFormatter, formatDocument, formatSelection } from "../shared/formatter";
import { Format, type FormatOptions } from "@snapper/wasm";

function getOptions(): FormatOptions {
  const maxWidth = parseInt(
    (document.getElementById("max-width") as HTMLInputElement).value,
    10,
  ) || 0;
  const lang = (document.getElementById("lang") as HTMLSelectElement).value;
  const abbrevsRaw = (document.getElementById("abbreviations") as HTMLInputElement).value;
  const extraAbbreviations = abbrevsRaw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);

  return {
    format: Format.Plaintext,
    maxWidth,
    lang,
    extraAbbreviations: extraAbbreviations.length > 0 ? extraAbbreviations : undefined,
  };
}

function setStatus(msg: string): void {
  const el = document.getElementById("status");
  if (el) el.textContent = msg;
}

Office.onReady(async () => {
  try {
    await initFormatter();
    setStatus("Ready");
  } catch (e) {
    setStatus("Failed to load WASM module");
    console.error(e);
    return;
  }

  document.getElementById("format-doc")?.addEventListener("click", async () => {
    setStatus("Formatting...");
    try {
      await formatDocument(getOptions());
      setStatus("Done");
    } catch (e) {
      setStatus("Error: " + String(e));
    }
  });

  document.getElementById("format-sel")?.addEventListener("click", async () => {
    setStatus("Formatting selection...");
    try {
      await formatSelection(getOptions());
      setStatus("Done");
    } catch (e) {
      setStatus("Error: " + String(e));
    }
  });
});
