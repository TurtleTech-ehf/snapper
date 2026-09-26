import { Plugin, MarkdownView } from "obsidian";
import {
  type SnapperSettings,
  DEFAULT_SETTINGS,
  SnapperSettingTab,
} from "./settings";
import { initFormatter, formatEditor, formatSelection } from "./formatter";

export default class SnapperPlugin extends Plugin {
  settings: SnapperSettings = DEFAULT_SETTINGS;

  async onload(): Promise<void> {
    await this.loadSettings();

    try {
      await initFormatter();
    } catch (e) {
      console.error("snapper: failed to initialize WASM module", e);
      return;
    }

    this.addCommand({
      id: "format-note",
      name: "Format entire note",
      editorCallback: (editor, view) => {
        const filename = view.file?.name;
        formatEditor(editor, this.settings, filename);
      },
    });

    this.addCommand({
      id: "format-selection",
      name: "Format selection",
      editorCallback: (editor, view) => {
        const filename = view.file?.name;
        formatSelection(editor, this.settings, filename);
      },
    });

    if (this.settings.formatOnSave) {
      this.registerEvent(
        this.app.vault.on("modify", (file) => {
          const view = this.app.workspace.getActiveViewOfType(MarkdownView);
          if (view && view.file === file) {
            formatEditor(view.editor, this.settings, file.name);
          }
        }),
      );
    }

    this.addSettingTab(new SnapperSettingTab(this.app, this));
  }

  async loadSettings(): Promise<void> {
    this.settings = Object.assign({}, DEFAULT_SETTINGS, await this.loadData());
  }

  async saveSettings(): Promise<void> {
    await this.saveData(this.settings);
  }
}
