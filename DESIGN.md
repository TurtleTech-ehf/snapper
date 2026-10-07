---
name: Snapper
description: Public page for the semantic line-break formatter.
colors:
  teal: "#004D40"
  teal-light: "#00695C"
  coral: "#FF655D"
  yellow: "#F1DB4B"
  background: "#FDFCF9"
  text: "#1a2e1f"
typography:
  body:
    fontFamily: "Jost, system-ui, sans-serif"
    fontWeight: 400
  mono:
    fontFamily: "JetBrains Mono, monospace"
    fontWeight: 400
rounded:
  logo: "28px"
  control: "8px"
  panel: "12px"
spacing:
  section-y: "6rem"
  section-x: "0"
components:
  button-primary:
    backgroundColor: "{colors.teal}"
    textColor: "{colors.background}"
---

# Design System: Snapper

## Overview

**Creative North Star: "The forest page"**

(Inferred from `site/index.html`.) The static page sets its own custom properties: deep teal, coral, cream, and forest greens, with Jost for text and JetBrains Mono for code.

**Key Characteristics:**

- Teal `#004D40`.
- Background `#FDFCF9`.
- Jost and JetBrains Mono.

## Colors

### Primary

- **Teal** (`#004D40`): `--teal`.

### Secondary

- **Coral** (`#FF655D`): `--coral`.
- **Yellow** (`#F1DB4B`): `--yellow`.

### Neutral

- **Paper** (`#FDFCF9`): `--bg`.
- **Ink** (`#1a2e1f`): `--text`.

## Typography

**Body Font:** Jost
**Label/Mono Font:** JetBrains Mono

### Hierarchy

- **Body**: `var(--sans)` on the page sections.
- **Label**: `var(--mono)` on code samples.

## Layout

`section` padding is `6rem 0` in `site/index.html`.

## Shapes

The logo uses a 28px radius. Controls use 8px. Panels and the demo close at 12px.

## Components

### Navigation

- Text uses `var(--sans)` and the teal custom properties.

## Do's and Don'ts

### Do:

- **Do** keep `#004D40` and Jost as written in `site/index.html`.

### Don't:

- **Don't** replace Jost with Atkinson Hyperlegible Next. This page does not load that face.
