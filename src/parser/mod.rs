pub mod latex;
pub mod markdown;
pub mod org;
pub mod pandoc;
pub mod plaintext;
pub mod rst;

/// A region of text classified by a format parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Region {
    /// Prose text that should be reflowed with semantic line breaks.
    Prose(String),
    /// Structural content that must pass through unchanged.
    Structure(String),
    /// Blank line(s) preserved as paragraph separators.
    BlankLines(String),
}

/// Trait for format-specific parsers that classify text into regions.
pub trait FormatParser {
    fn parse(&self, input: &str) -> Vec<Region>;
}

/// Create the appropriate parser for a given format.
pub fn parser_for_format(format: crate::format::Format) -> Box<dyn FormatParser> {
    use crate::format::Format;
    match format {
        Format::Org => Box::new(org::OrgParser),
        Format::Latex => Box::new(latex::LatexParser),
        Format::Markdown => Box::new(markdown::MarkdownParser),
        Format::Rst => Box::new(rst::RstParser),
        Format::Plaintext => Box::new(plaintext::PlaintextParser),
    }
}

/// Flush accumulated prose into the region list, clearing the buffer.
pub fn flush_prose(prose: &mut String, regions: &mut Vec<Region>) {
    if !prose.is_empty() {
        regions.push(Region::Prose(prose.clone()));
        prose.clear();
    }
}

/// Check if a line contains a snapper pragma.
/// Returns Some(false) for "snapper:off", Some(true) for "snapper:on", None otherwise.
pub fn check_pragma(line: &str) -> Option<bool> {
    let trimmed = line.trim();
    // Strip format-specific comment markers
    let content = trimmed
        .strip_prefix("# ") // Org comment
        .or_else(|| trimmed.strip_prefix("% ")) // LaTeX comment
        .or_else(|| {
            // HTML/Markdown comment
            trimmed
                .strip_prefix("<!-- ")
                .and_then(|s| s.strip_suffix(" -->"))
        })
        .unwrap_or(trimmed); // Plaintext: bare pragma
    let content = content.trim();
    if content == "snapper:off" {
        Some(false)
    } else if content == "snapper:on" {
        Some(true)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pragma_org_comment() {
        assert_eq!(check_pragma("# snapper:off"), Some(false));
        assert_eq!(check_pragma("# snapper:on"), Some(true));
    }

    #[test]
    fn pragma_latex_comment() {
        assert_eq!(check_pragma("% snapper:off"), Some(false));
        assert_eq!(check_pragma("% snapper:on"), Some(true));
    }

    #[test]
    fn pragma_html_comment() {
        assert_eq!(check_pragma("<!-- snapper:off -->"), Some(false));
        assert_eq!(check_pragma("<!-- snapper:on -->"), Some(true));
    }

    #[test]
    fn pragma_bare() {
        assert_eq!(check_pragma("snapper:off"), Some(false));
        assert_eq!(check_pragma("snapper:on"), Some(true));
    }

    #[test]
    fn pragma_none() {
        assert_eq!(check_pragma("regular text"), None);
        assert_eq!(check_pragma("# a comment"), None);
        assert_eq!(check_pragma(""), None);
    }
}
