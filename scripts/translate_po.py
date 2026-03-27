#!/usr/bin/env python3
"""Translate empty msgstr entries in .po files using a dictionary approach.

Usage: python translate_po.py <lang> <pofile> [<pofile> ...]
Translates in-place. Only fills empty msgstr entries.
"""
import re
import sys
from pathlib import Path

# Translation dictionaries for UI/nav strings and common phrases
IS_DICT = {
    # Section headers
    "Quickstart": "Byrjun",
    "Installation": "Uppsetning",
    "Basic usage": "Grunnnotkun",
    "What it does": "Hvad gerir thad",
    "Format detection": "Snidgreining",
    "CI integration": "CI samthaetting",
    "Pre-commit hook": "Pre-commit hook",
    "Emacs (Apheleia)": "Emacs (Apheleia)",
    "Editor Integration": "Samthaetting vid ritla",
    "CI Enforcement": "CI-framfylgd",
    "Git Smudge/Clean Filter": "Git smudge/clean sia",
    "Vale Integration": "Vale samthaetting",
    "CLI Reference": "CLI tilvisun",
    "Synopsis": "Yfirlit",
    "Options": "Valkostir",
    "Exit codes": "Utgangskodar",
    "Abbreviation Handling": "Medhoendlun skammstofana",
    "Configuration": "Stillingar",
    "Supported Formats": "Studd snid",
    "Contributing": "Ad leggja til",
    "Development setup": "Throunaruppsetning",
    "Running checks": "Keyra profanir",
    "Project structure": "Verkefnisbygging",
    "Commit conventions": "Commit-venjur",
    "Building documentation": "Byggja skjolun",
    "Adding a new format parser": "Baeta vid nyju snidthattara",
    "Changelog": "Breytingaskra",
    "Project config file": "Stillingarskra verkefnis",
    "Config format": "Snid stillingarskrar",
    "Fields": "Svid",
    "Precedence": "Forgangur",
    "Overview": "Yfirlit",
    "Style package setup": "Uppsetning stilpakka",
    "Bundled rules": "Medfylgjandi reglur",
    "Precise CI enforcement": "Naekvam CI-framfylgd",
    "Combining vale and snapper": "Sameina vale og snapper",
    "Setup": "Uppsetning",
    "Activate via .gitattributes": "Virkja med .gitattributes",
    "Per-format filters": "Siu eftir snidi",
    "Verifying the filter works": "Staddfesta ad sian virki",
    "How abbreviation detection works": "Hvernig skammstofunargreining virkar",
    "Built-in abbreviations": "Innbyggdar skammstafanir",
    "Titles and honorifics": "Titlar og virulegir titlar",
    "Academic and scientific": "Fraedileg og visindaleg",
    "Latin": "Latneskt",
    "Time and dates": "Timi og dagsetningar",
    "Common": "Algengt",
    "Single-letter initials": "Eins stafs upphafsstafir",
    "Adding project-specific abbreviations": "Baeta vid verkefnasertaekum skammstofunum",
    "Inline token protection": "Vorn innleidds takns",
    "Sentence Detection": "Setningargreining",
    # Common phrases
    "From source:": "Fra frumkoda:",
    "Or build from source:": "Eda byggt fra frumkoda:",
    "With Nix:": "Med Nix:",
    "Vim / Neovim": "Vim / Neovim",
    "VS Code": "VS Code",
    "Generic (any editor)": "Almennt (hvaða ritill sem er)",
    "GitHub Actions": "GitHub Actions",
    "GitLab CI": "GitLab CI",
    "Makefile integration": "Makefile samthæetting",
    "Org-mode": "Org-mode",
    "LaTeX": "LaTeX",
    "Markdown": "Markdown",
    "Plaintext": "Hreinntexti",
}

PL_DICT = {
    # Section headers
    "Quickstart": "Szybki start",
    "Installation": "Instalacja",
    "Basic usage": "Podstawowe uzycie",
    "What it does": "Co robi",
    "Format detection": "Wykrywanie formatu",
    "CI integration": "Integracja z CI",
    "Pre-commit hook": "Hook pre-commit",
    "Emacs (Apheleia)": "Emacs (Apheleia)",
    "Editor Integration": "Integracja z edytorem",
    "CI Enforcement": "Wymuszanie w CI",
    "Git Smudge/Clean Filter": "Filtr git smudge/clean",
    "Vale Integration": "Integracja z vale",
    "CLI Reference": "Dokumentacja CLI",
    "Synopsis": "Skladnia",
    "Options": "Opcje",
    "Exit codes": "Kody wyjscia",
    "Abbreviation Handling": "Obsluga skrotow",
    "Configuration": "Konfiguracja",
    "Supported Formats": "Obslugiwane formaty",
    "Contributing": "Wspoltworzenie",
    "Development setup": "Konfiguracja srodowiska",
    "Running checks": "Uruchamianie sprawdzen",
    "Project structure": "Struktura projektu",
    "Commit conventions": "Konwencje commitow",
    "Building documentation": "Budowanie dokumentacji",
    "Adding a new format parser": "Dodawanie nowego parsera formatu",
    "Changelog": "Historia zmian",
    "Project config file": "Plik konfiguracji projektu",
    "Config format": "Format konfiguracji",
    "Fields": "Pola",
    "Precedence": "Kolejnosc priorytetow",
    "Overview": "Przeglad",
    "Style package setup": "Konfiguracja pakietu stylow",
    "Bundled rules": "Dolaczone reguly",
    "Precise CI enforcement": "Precyzyjne wymuszanie w CI",
    "Combining vale and snapper": "Laczenie vale i snapper",
    "Setup": "Konfiguracja",
    "Activate via .gitattributes": "Aktywacja przez .gitattributes",
    "Per-format filters": "Filtry wedlug formatu",
    "Verifying the filter works": "Weryfikacja dzialania filtra",
    "How abbreviation detection works": "Jak dziala wykrywanie skrotow",
    "Built-in abbreviations": "Wbudowane skroty",
    "Titles and honorifics": "Tytuly i zwroty grzecznosciowe",
    "Academic and scientific": "Naukowe",
    "Latin": "Lacinskie",
    "Time and dates": "Czas i daty",
    "Common": "Popularne",
    "Single-letter initials": "Jednoliterowe inicjaly",
    "Adding project-specific abbreviations": "Dodawanie skrotow specyficznych dla projektu",
    "Inline token protection": "Ochrona tokenow inline",
    "Sentence Detection": "Wykrywanie zdan",
    # Common phrases
    "From source:": "Ze zrodel:",
    "Or build from source:": "Lub zbuduj ze zrodel:",
    "With Nix:": "Z Nix:",
    "Vim / Neovim": "Vim / Neovim",
    "VS Code": "VS Code",
    "Generic (any editor)": "Ogolne (dowolny edytor)",
    "GitHub Actions": "GitHub Actions",
    "GitLab CI": "GitLab CI",
    "Makefile integration": "Integracja z Makefile",
    "Org-mode": "Org-mode",
    "LaTeX": "LaTeX",
    "Markdown": "Markdown",
    "Plaintext": "Zwykly tekst",
}

# Longer prose translations (msgid -> {lang: msgstr})
PROSE = {
    # Quickstart
    "Format a file, printing to stdout:": {
        "is": "Snidadu skra, skrifa a stdout:",
        "pl": "Sformatuj plik, wypisz na stdout:",
    },
    "Format in place:": {
        "is": "Snidadu a stadnum:",
        "pl": "Sformatuj w miejscu:",
    },
    "Pipe through stdin (for editor integration):": {
        "is": "Sigtu i gegnum stdin (fyrir ritilssamthattingu):",
        "pl": "Przeslij przez stdin (dla integracji z edytorem):",
    },
    "snapper auto-detects the format from the file extension:": {
        "is": "snapper greinir sjálfkrafa snidid fra skraarendingu:",
        "pl": "snapper automatycznie wykrywa format z rozszerzenia pliku:",
    },
    "Override with ``--format``:": {
        "is": "Hneka med ``--format``:",
        "pl": "Nadpisz za pomoca ``--format``:",
    },
    "This runs ``snapper --in-place`` on changed files matching ``.org``, ``.tex``, ``.md``, and ``.txt``.": {
        "is": "Thetta keyrir ``snapper --in-place`` a breyttum skram sem passa vid ``.org``, ``.tex``, ``.md`` og ``.txt``.",
        "pl": "Uruchamia ``snapper --in-place`` na zmienionych plikach pasujacych do ``.org``, ``.tex``, ``.md`` i ``.txt``.",
    },
}


def translate_po(lang, path):
    d = IS_DICT if lang == "is" else PL_DICT
    text = Path(path).read_text(encoding="utf-8")

    # Remove fuzzy flag from header
    text = text.replace("#, fuzzy\n", "", 1)

    # Update header
    text = re.sub(r'PO-Revision-Date: .*\n', 'PO-Revision-Date: 2026-03-27 23:00+0100\\n"\n', text)
    text = re.sub(r'Last-Translator: .*\n', 'Last-Translator: Rohit Goswami <rgoswami@ieee.org>\\n"\n', text)

    # Split into entries
    entries = re.split(r'\n(?=#:)', text)
    result = [entries[0]]  # header

    for entry in entries[1:]:
        # Extract msgid
        m = re.search(r'msgid "(.*?)"(?:\n"(.*?)")*', entry, re.DOTALL)
        if not m:
            result.append(entry)
            continue

        # Get full msgid text
        msgid_lines = re.findall(r'msgid "(.*?)"\n|^"(.*?)"\n', entry, re.MULTILINE)
        msgid_raw = entry[entry.index('msgid '):]
        msgid_match = re.search(r'msgid\s+"((?:[^"\\]|\\.)*)"\s*(?:\n"((?:[^"\\]|\\.)*)")*', msgid_raw)

        # Simple: get the msgid content
        msgid_start = entry.index('msgid "') + 7
        msgstr_pos = entry.index('\nmsgstr ')
        msgid_block = entry[entry.index('msgid '):msgstr_pos]
        # Extract actual text from msgid block
        parts = re.findall(r'"((?:[^"\\]|\\.)*)"', msgid_block)
        msgid_text = "".join(parts)

        # Check if msgstr is empty
        msgstr_block = entry[msgstr_pos + 1:]
        msgstr_parts = re.findall(r'"((?:[^"\\]|\\.)*)"', msgstr_block)
        msgstr_text = "".join(msgstr_parts)

        if msgstr_text:  # already translated
            result.append(entry)
            continue

        # Look up translation
        trans = None
        if msgid_text in d:
            trans = d[msgid_text]
        elif msgid_text in PROSE and lang in PROSE[msgid_text]:
            trans = PROSE[msgid_text][lang]

        if trans:
            entry = entry[:msgstr_pos] + f'\nmsgstr "{trans}"\n'
        result.append(entry)

    Path(path).write_text("\n".join(result), encoding="utf-8")
    # Count remaining empty
    remaining = text.count('msgstr ""\n') - 1  # minus header
    print(f"  {path}: translated dict matches, ~{remaining} strings remain for manual review")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: translate_po.py <is|pl> <file.po> [...]")
        sys.exit(1)
    lang = sys.argv[1]
    for f in sys.argv[2:]:
        translate_po(lang, f)
