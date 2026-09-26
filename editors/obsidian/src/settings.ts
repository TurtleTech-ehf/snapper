import { App, PluginSettingTab, Setting } from "obsidian";
import type SnapperPlugin from "./main";

export interface SnapperSettings {
  formatOnSave: boolean;
  maxWidth: number;
  lang: string;
  extraAbbreviations: string;
}

export const DEFAULT_SETTINGS: SnapperSettings = {
  formatOnSave: false,
  maxWidth: 0,
  lang: "en",
  extraAbbreviations: "",
};

export class SnapperSettingTab extends PluginSettingTab {
  plugin: SnapperPlugin;

  constructor(app: App, plugin: SnapperPlugin) {
    super(app, plugin);
    this.plugin = plugin;
  }

  display(): void {
    const { containerEl } = this;
    containerEl.empty();

    new Setting(containerEl)
      .setName("Format on save")
      .setDesc("Automatically format notes when saving.")
      .addToggle((toggle) =>
        toggle
          .setValue(this.plugin.settings.formatOnSave)
          .onChange(async (value) => {
            this.plugin.settings.formatOnSave = value;
            await this.plugin.saveSettings();
          }),
      );

    new Setting(containerEl)
      .setName("Max line width")
      .setDesc("Wrap sentences exceeding this width (0 = unlimited).")
      .addText((text) =>
        text
          .setPlaceholder("0")
          .setValue(String(this.plugin.settings.maxWidth))
          .onChange(async (value) => {
            this.plugin.settings.maxWidth = parseInt(value, 10) || 0;
            await this.plugin.saveSettings();
          }),
      );

    new Setting(containerEl)
      .setName("Language")
      .setDesc("Language for abbreviation handling (en, de, fr, is, pl).")
      .addDropdown((dropdown) =>
        dropdown
          .addOptions({ en: "English", de: "German", fr: "French", is: "Icelandic", pl: "Polish" })
          .setValue(this.plugin.settings.lang)
          .onChange(async (value) => {
            this.plugin.settings.lang = value;
            await this.plugin.saveSettings();
          }),
      );

    new Setting(containerEl)
      .setName("Extra abbreviations")
      .setDesc("Comma-separated list of project-specific abbreviations.")
      .addText((text) =>
        text
          .setPlaceholder("GROMACS, LAMMPS, DFT")
          .setValue(this.plugin.settings.extraAbbreviations)
          .onChange(async (value) => {
            this.plugin.settings.extraAbbreviations = value;
            await this.plugin.saveSettings();
          }),
      );
  }
}
