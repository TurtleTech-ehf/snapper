//! Integration tests for the code-block comment-reflow path.
//!
//! Each test feeds a synthetic input through `format_text` with a
//! synthesised `FormatConfig` whose `code` table carries the relevant
//! language entry. We do not depend on `.snapperrc.toml` loading here; the
//! config-roundtrip path is covered by unit tests in `src/config.rs`.
//!
//! Idempotence is asserted alongside each functional assertion: the
//! `assert_round_trip!` macro applies `format_text` twice and requires the
//! second pass to be a byte-identical no-op.

use std::collections::HashMap;

use snapper_fmt::FormatConfig;
use snapper_fmt::config::CodeLang;
use snapper_fmt::format::Format;
use snapper_fmt::format_text;

/// Build a per-language map keyed by the language strings used on code
/// fences. Caller supplies (lang, line_comment, block_comment) tuples.
#[allow(clippy::type_complexity)]
fn code_map(entries: &[(&str, Option<&str>, Option<[&str; 2]>)]) -> HashMap<String, CodeLang> {
    let mut map = HashMap::new();
    for (lang, lc, bc) in entries {
        map.insert(
            (*lang).to_string(),
            CodeLang {
                line_comment: lc.map(|s| s.to_string()),
                block_comment: bc.map(|pair| [pair[0].to_string(), pair[1].to_string()]),
                formatter: None,
            },
        );
    }
    map
}

fn config(format: Format, code: HashMap<String, CodeLang>) -> FormatConfig {
    FormatConfig {
        format,
        code,
        ..Default::default()
    }
}

/// Round-trip helper. Returns the once-formatted output and asserts the
/// twice-formatted output is byte-identical to it (idempotence).
fn round_trip(input: &str, cfg: &FormatConfig) -> String {
    let first = format_text(input, cfg).expect("first format pass succeeded");
    let second = format_text(&first, cfg).expect("second format pass succeeded");
    assert_eq!(
        first, second,
        "format_text(format_text(input)) must equal format_text(input)"
    );
    first
}

// ---------------------------------------------------------------------------
// R5: line-comment reflow on four languages
// ---------------------------------------------------------------------------

#[test]
fn rust_line_comment_reflows_two_sentences() {
    let input = "\
```rust
// Long comment with two sentences. Second sentence here.
fn main() {}
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    let expected = "\
```rust
// Long comment with two sentences.
// Second sentence here.
fn main() {}
```
";
    assert_eq!(out, expected);
}

#[test]
fn python_line_comment_reflows() {
    let input = "\
```python
# First sentence. Second sentence.
print('hi')
```
";
    let cfg = config(Format::Markdown, code_map(&[("python", Some("#"), None)]));
    let out = round_trip(input, &cfg);
    assert!(out.contains("# First sentence.\n# Second sentence.\n"));
}

#[test]
fn lua_line_comment_reflows() {
    let input = "\
```lua
-- First sentence. Second sentence.
print('hi')
```
";
    let cfg = config(Format::Markdown, code_map(&[("lua", Some("--"), None)]));
    let out = round_trip(input, &cfg);
    assert!(out.contains("-- First sentence.\n-- Second sentence.\n"));
}

#[test]
fn lisp_line_comment_reflows() {
    let input = "\
```lisp
; First sentence. Second sentence.
(print 1)
```
";
    let cfg = config(Format::Markdown, code_map(&[("lisp", Some(";"), None)]));
    let out = round_trip(input, &cfg);
    assert!(out.contains("; First sentence.\n; Second sentence.\n"));
}

// ---------------------------------------------------------------------------
// R7: block-comment reflow on at least three of {rust, python, lua, html}
// ---------------------------------------------------------------------------

#[test]
fn rust_block_comment_one_liner_splits() {
    let input = "\
```rust
/* Two sentences. Like this. */
fn x() {}
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    // Opening and closing markers stay on their own lines; interior
    // reflows as plaintext.
    assert!(out.contains("/*\n Two sentences.\n Like this.\n*/\n"));
}

#[test]
fn python_block_comment_one_liner_splits() {
    let input = "\
```python
\"\"\" First. Second. \"\"\"
def f(): pass
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("python", Some("#"), Some(["\"\"\"", "\"\"\""]))]),
    );
    let out = round_trip(input, &cfg);
    assert!(out.contains("\"\"\"\n First.\n Second.\n\"\"\"\n"));
}

#[test]
fn html_block_comment_one_liner_splits() {
    let input = "\
```html
<!-- First. Second. -->
<p>hi</p>
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("html", None, Some(["<!--", "-->"]))]),
    );
    let out = round_trip(input, &cfg);
    assert!(out.contains("<!--\n First.\n Second.\n-->\n"));
}

// ---------------------------------------------------------------------------
// R8: indentation preservation -- flush-left and 4-space-indented fences
// ---------------------------------------------------------------------------

#[test]
fn flush_left_fence_preserves_indent() {
    let input = "\
```rust
    // First sentence here. Second one too.
    fn x() {}
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    // 4-space body indent preserved on every output comment line.
    assert!(out.contains("    // First sentence here.\n    // Second one too.\n    fn x() {}\n"));
}

#[test]
fn indented_fence_preserves_body_indent() {
    // Markdown list-nested fence: the fence itself is 4-space-indented,
    // and so is the body. Markdown parser's fence detection trim_starts so
    // it still recognises the fence, and the body's leading whitespace
    // round-trips byte-identical.
    let input = "\
- An item:

    ```rust
    // First. Second.
    fn x() {}
    ```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    assert!(out.contains("    // First.\n    // Second.\n    fn x() {}\n"));
}

// ---------------------------------------------------------------------------
// R15: pragma respect inside code blocks for three languages
// ---------------------------------------------------------------------------

#[test]
fn rust_pragma_freezes_section() {
    let input = "\
```rust
// snapper:off
// Long.
// Off.
// snapper:on
// Reflow this. Now.
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    let expected_body = "\
// snapper:off
// Long.
// Off.
// snapper:on
// Reflow this.
// Now.
";
    assert!(out.contains(expected_body), "got:\n{out}");
}

#[test]
fn python_pragma_freezes_section() {
    let input = "\
```python
# snapper:off
# Long.
# Off.
# snapper:on
# Reflow this. Now.
```
";
    let cfg = config(Format::Markdown, code_map(&[("python", Some("#"), None)]));
    let out = round_trip(input, &cfg);
    let expected_body = "\
# snapper:off
# Long.
# Off.
# snapper:on
# Reflow this.
# Now.
";
    assert!(out.contains(expected_body));
}

#[test]
fn lua_pragma_freezes_section() {
    let input = "\
```lua
-- snapper:off
-- Long.
-- Off.
-- snapper:on
-- Reflow this. Now.
```
";
    let cfg = config(Format::Markdown, code_map(&[("lua", Some("--"), None)]));
    let out = round_trip(input, &cfg);
    let expected_body = "\
-- snapper:off
-- Long.
-- Off.
-- snapper:on
-- Reflow this.
-- Now.
";
    assert!(out.contains(expected_body));
}

// ---------------------------------------------------------------------------
// R6: the .> clipping matrix -- both broken-today shapes and regression
// guards. All eight forms must round-trip byte-identical when embedded in
// a Rust comment line at the bottom of a code block.
// ---------------------------------------------------------------------------

fn matrix_input(tail: &str) -> String {
    format!("```rust\nfn main() {{}}\n// e.g. {tail}\n```\n",)
}

#[test]
fn clipping_matrix_round_trips() {
    // Four BROKEN-today shapes.
    let broken = ["Vec<...>", "<.>", "<..>", "<a.>"];
    // Four WORKING-today shapes (regression guard).
    let working = ["<a.b>", "<.b>", "<a>", "<>"];

    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    for tail in broken.iter().chain(working.iter()) {
        let input = matrix_input(tail);
        let out = round_trip(&input, &cfg);
        assert_eq!(
            out, input,
            "shape {tail:?} must round-trip; got:\n{out}\n!=\n{input}",
        );
    }
}

// ---------------------------------------------------------------------------
// R2: org and rst parsers also produce Region::Code that the reflow path
// recognises. We assert end-to-end behaviour without inspecting the AST.
// ---------------------------------------------------------------------------

#[test]
fn org_src_block_comment_reflows() {
    let input = "\
#+BEGIN_SRC rust
// First sentence. Second sentence.
fn main() {}
#+END_SRC
";
    let cfg = config(
        Format::Org,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    assert!(out.contains("// First sentence.\n// Second sentence.\nfn main() {}"));
}

#[test]
fn rst_code_block_comment_reflows() {
    let input = "\
.. code-block:: python

   # First sentence. Second sentence.
   print('hi')
";
    let cfg = config(Format::Rst, code_map(&[("python", Some("#"), None)]));
    let out = round_trip(input, &cfg);
    // RST body indent (3 spaces) is preserved on every reflowed comment line.
    assert!(
        out.contains("   # First sentence.\n   # Second sentence."),
        "got:\n{out}"
    );
}

// ---------------------------------------------------------------------------
// Single language passthrough when [code] entry is absent: the block body
// must be byte-identical to the input. Idempotence assertion is implicit.
// ---------------------------------------------------------------------------

#[test]
fn unknown_lang_passes_through_unchanged() {
    let input = "\
```ocaml
(* Two sentences. They stay together. *)
let x = 1
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    assert_eq!(out, input);
}

#[test]
fn no_lang_on_fence_passes_through_unchanged() {
    let input = "\
```
// looks like rust but no fence lang -> verbatim. Stays together.
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    assert_eq!(out, input);
}

// ---------------------------------------------------------------------------
// Grammar-backed discovery: comments that do not open their line, and
// markers that only look like comments. Both need a parse to tell apart.
// ---------------------------------------------------------------------------

#[test]
#[cfg(feature = "treesitter")]
fn rust_trailing_comment_reflows_aligned_under_itself() {
    let input = "\
```rust
fn main() {
    let n = 1; // Trailing note. Second sentence here.
}
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    let expected = "\
```rust
fn main() {
    let n = 1; // Trailing note.
               // Second sentence here.
}
```
";
    assert_eq!(out, expected);
}

#[test]
#[cfg(feature = "treesitter")]
fn rust_marker_inside_string_literal_is_not_a_comment() {
    let input = "\
```rust
fn main() {
    let s = \"// not a comment. Really not one.\";
}
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    assert_eq!(out, input);
}

#[test]
#[cfg(feature = "treesitter")]
fn rust_doc_comment_keeps_its_own_marker() {
    let input = "\
```rust
/// Doc line one. Doc line two.
fn x() {}
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    assert!(
        out.contains("/// Doc line one.\n/// Doc line two.\n"),
        "doc marker must survive the split, got:\n{out}"
    );
}

#[test]
#[cfg(feature = "treesitter")]
fn python_trailing_comment_reflows() {
    let input = "\
```python
x = 1  # First sentence. Second sentence.
```
";
    let cfg = config(Format::Markdown, code_map(&[("python", Some("#"), None)]));
    let out = round_trip(input, &cfg);
    assert!(
        out.contains("x = 1  # First sentence.\n       # Second sentence.\n"),
        "trailing python comment must align under itself, got:\n{out}"
    );
}

#[test]
#[cfg(feature = "treesitter")]
fn pragma_freezes_a_trailing_comment() {
    let input = "\
```rust
// snapper:off
fn main() {
    let n = 1; // Frozen note. Stays on one line.
}
// snapper:on
```
";
    let cfg = config(
        Format::Markdown,
        code_map(&[("rust", Some("//"), Some(["/*", "*/"]))]),
    );
    let out = round_trip(input, &cfg);
    assert_eq!(out, input);
}

#[test]
fn language_without_a_grammar_still_reflows_own_line_comments() {
    let input = "\
```lua
-- First sentence. Second sentence.
local x = 1 -- trailing stays put. Second here.
```
";
    let cfg = config(Format::Markdown, code_map(&[("lua", Some("--"), None)]));
    let out = round_trip(input, &cfg);
    assert!(
        out.contains("-- First sentence.\n-- Second sentence.\n"),
        "scanner path must still split an own-line comment, got:\n{out}"
    );
    assert!(
        out.contains("local x = 1 -- trailing stays put. Second here.\n"),
        "no grammar for lua, so a trailing comment is left alone, got:\n{out}"
    );
}
