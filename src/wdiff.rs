use imara_diff::{Algorithm, Diff, InternedInput};
use unicode_segmentation::UnicodeSegmentation;

/// Word-level diff status for rendering.
#[derive(Debug, Clone, PartialEq)]
pub enum WordStatus {
    Unchanged(String),
    Added(String),
    Deleted(String),
}

/// Compute word-level diff between two sentences.
pub fn word_diff(old: &str, new: &str) -> Vec<WordStatus> {
    let old_words: Vec<&str> = old.unicode_words().collect();
    let new_words: Vec<&str> = new.unicode_words().collect();

    if old_words == new_words {
        return old_words
            .into_iter()
            .map(|w| WordStatus::Unchanged(w.to_string()))
            .collect();
    }

    // Use imara-diff on words (join with newlines so each word is a "line")
    let old_lines = old_words.join("\n");
    let new_lines = new_words.join("\n");

    let input = InternedInput::new(old_lines.as_str(), new_lines.as_str());
    let diff = Diff::compute(Algorithm::Histogram, &input);

    let mut result = Vec::new();

    let old_removed: Vec<bool> = (0..old_words.len())
        .map(|i| diff.is_removed(i as u32))
        .collect();
    let new_added: Vec<bool> = (0..new_words.len())
        .map(|i| diff.is_added(i as u32))
        .collect();

    let mut oi = 0;
    let mut ni = 0;

    while oi < old_words.len() || ni < new_words.len() {
        if oi < old_words.len() && old_removed[oi] {
            result.push(WordStatus::Deleted(old_words[oi].to_string()));
            oi += 1;
        } else if ni < new_words.len() && new_added[ni] {
            result.push(WordStatus::Added(new_words[ni].to_string()));
            ni += 1;
        } else if oi < old_words.len() && ni < new_words.len() {
            // Both unchanged -- should be the same word
            result.push(WordStatus::Unchanged(new_words[ni].to_string()));
            oi += 1;
            ni += 1;
        } else if oi < old_words.len() {
            result.push(WordStatus::Deleted(old_words[oi].to_string()));
            oi += 1;
        } else {
            result.push(WordStatus::Added(new_words[ni].to_string()));
            ni += 1;
        }
    }

    result
}

/// Render word diff for terminal (ANSI colors).
pub fn render_terminal(words: &[WordStatus]) -> String {
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match w {
            WordStatus::Unchanged(s) => out.push_str(s),
            WordStatus::Deleted(s) => {
                out.push_str("\x1b[9;31m"); // red strikethrough
                out.push_str(s);
                out.push_str("\x1b[0m");
            }
            WordStatus::Added(s) => {
                out.push_str("\x1b[1;32m"); // green bold
                out.push_str(s);
                out.push_str("\x1b[0m");
            }
        }
    }
    out
}

/// Render word diff as plaintext markup.
pub fn render_plaintext(words: &[WordStatus]) -> String {
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match w {
            WordStatus::Unchanged(s) => out.push_str(s),
            WordStatus::Deleted(s) => {
                out.push_str("[-");
                out.push_str(s);
                out.push_str("-]");
            }
            WordStatus::Added(s) => {
                out.push_str("{+");
                out.push_str(s);
                out.push_str("+}");
            }
        }
    }
    out
}

/// Render word diff as LaTeX (latexdiff-compatible).
pub fn render_latex(words: &[WordStatus]) -> String {
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match w {
            WordStatus::Unchanged(s) => out.push_str(s),
            WordStatus::Deleted(s) => {
                out.push_str("\\DIFdel{");
                out.push_str(s);
                out.push('}');
            }
            WordStatus::Added(s) => {
                out.push_str("\\DIFadd{");
                out.push_str(s);
                out.push('}');
            }
        }
    }
    out
}

/// Render word diff as Markdown.
pub fn render_markdown(words: &[WordStatus]) -> String {
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match w {
            WordStatus::Unchanged(s) => out.push_str(s),
            WordStatus::Deleted(s) => {
                out.push_str("~~");
                out.push_str(s);
                out.push_str("~~");
            }
            WordStatus::Added(s) => {
                out.push_str("**");
                out.push_str(s);
                out.push_str("**");
            }
        }
    }
    out
}

/// Render word diff as Org-mode.
pub fn render_org(words: &[WordStatus]) -> String {
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match w {
            WordStatus::Unchanged(s) => out.push_str(s),
            WordStatus::Deleted(s) => {
                out.push('+');
                out.push_str(s);
                out.push('+');
            }
            WordStatus::Added(s) => {
                out.push('*');
                out.push_str(s);
                out.push('*');
            }
        }
    }
    out
}

/// Render word diff as typst.
pub fn render_typst(words: &[WordStatus]) -> String {
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match w {
            WordStatus::Unchanged(s) => out.push_str(s),
            WordStatus::Deleted(s) => {
                out.push_str("#strike[");
                out.push_str(s);
                out.push(']');
            }
            WordStatus::Added(s) => {
                out.push_str("#highlight[");
                out.push_str(s);
                out.push(']');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_sentences() {
        let result = word_diff("Hello world", "Hello world");
        assert!(result.iter().all(|w| matches!(w, WordStatus::Unchanged(_))));
    }

    #[test]
    fn one_word_changed() {
        let result = word_diff(
            "This approach dramatically reduces noise",
            "This approach significantly reduces noise",
        );
        // "dramatically" -> deleted, "significantly" -> added, rest unchanged
        assert!(
            result
                .iter()
                .any(|w| matches!(w, WordStatus::Deleted(s) if s == "dramatically"))
        );
        assert!(
            result
                .iter()
                .any(|w| matches!(w, WordStatus::Added(s) if s == "significantly"))
        );
    }

    #[test]
    fn render_plaintext_output() {
        let result = word_diff("old word here", "new word here");
        let rendered = render_plaintext(&result);
        assert!(rendered.contains("[-old-]"));
        assert!(rendered.contains("{+new+}"));
        assert!(rendered.contains("word"));
        assert!(rendered.contains("here"));
    }

    #[test]
    fn render_latex_output() {
        let result = word_diff("dramatically reduces", "significantly reduces");
        let rendered = render_latex(&result);
        assert!(rendered.contains("\\DIFdel{dramatically}"));
        assert!(rendered.contains("\\DIFadd{significantly}"));
        assert!(rendered.contains("reduces"));
    }

    #[test]
    fn word_added_at_end() {
        let result = word_diff("Hello world", "Hello world today");
        assert!(
            result
                .iter()
                .any(|w| matches!(w, WordStatus::Added(s) if s == "today"))
        );
    }

    #[test]
    fn word_deleted_from_middle() {
        let result = word_diff("Hello beautiful world", "Hello world");
        assert!(
            result
                .iter()
                .any(|w| matches!(w, WordStatus::Deleted(s) if s == "beautiful"))
        );
    }
}
