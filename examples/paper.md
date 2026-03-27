---
title: Semantic Line Breaking in Practice
author: A. Researcher
date: 2026-01-15
---

# Introduction

This document demonstrates semantic line breaking for Markdown files. When collaborating on documentation, blog posts, or papers written in Markdown, traditional wrapping causes noisy diffs. By placing each sentence on its own line, reviewers can see exactly what changed.

## The Problem

Consider editing a paragraph where you change one word. With traditional wrapping at 80 columns, the entire paragraph may reflow, producing a diff that touches every line. With semantic line breaks, only the changed sentence appears in the diff. This is especially valuable when using GitHub pull requests for review.

## How It Works

The formatter identifies prose regions and structural elements separately. Code blocks, front matter, and other structural elements pass through unchanged.

```python
# This code block is preserved exactly as-is
def example():
    return "semantic line breaks"
```

After the code block, prose continues with sentence-level formatting. The tool handles abbreviations like Dr. Smith, Fig. 3, and Latin terms like e.g. and i.e. correctly.

### Integration

- Use as a pre-commit hook for automatic formatting
- Add to CI with `--check` mode to enforce the convention
- Configure Emacs via Apheleia for format-on-save
- Pipe through stdin/stdout for editor integration

## Conclusion

Semantic line breaks are a simple convention that dramatically improves the collaboration experience on prose documents. This tool makes adoption painless across Markdown, LaTeX, Org-mode, and plaintext.
