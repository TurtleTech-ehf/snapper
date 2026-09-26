import { initFormatter, formatDocument, formatSelection } from "../shared/formatter";

let initialized = false;

async function ensureInit(): Promise<void> {
  if (!initialized) {
    await initFormatter();
    initialized = true;
  }
}

Office.onReady(async () => {
  await ensureInit();
});

/** Command handler: format entire document. */
async function formatDocumentCommand(event: Office.AddinCommands.Event): Promise<void> {
  await ensureInit();
  try {
    await formatDocument();
  } catch (e) {
    console.error("snapper: format document failed", e);
  }
  event.completed();
}

/** Command handler: format selection. */
async function formatSelectionCommand(event: Office.AddinCommands.Event): Promise<void> {
  await ensureInit();
  try {
    await formatSelection();
  } catch (e) {
    console.error("snapper: format selection failed", e);
  }
  event.completed();
}

// Register command handlers for ribbon buttons.
Office.actions.associate("FormatDocument", formatDocumentCommand);
Office.actions.associate("FormatSelection", formatSelectionCommand);
