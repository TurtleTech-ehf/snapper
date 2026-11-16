//! A Markdown paragraph with no blank line must not rescan itself per line.
//!
//! `DlLookahead` classifies each line once. Two thousand, eight thousand,
//! and thirty-two thousand sentences in one paragraph have to stay in a
//! linear band: four times the lines may take up to twelve times as long,
//! plus a short fixed slack so a busy machine does not fail the check.

use std::hint::black_box;
use std::time::{Duration, Instant};

use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn paragraph(sentences: usize) -> String {
    let line = "The quick brown fox jumps over the lazy dog near the riverbank.\n";
    line.repeat(sentences)
}

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn time_parse(input: &str) -> Duration {
    let mut best = Duration::MAX;
    for _ in 0..3 {
        let started = Instant::now();
        let spanned = MarkdownParser.parse_full(black_box(input));
        let elapsed = started.elapsed();
        let prose = spanned.iter().find_map(|region| match &region.region {
            Region::Prose(text) => Some(text.as_str()),
            _ => None,
        });
        let prose = prose.expect("paragraph is prose");
        assert_eq!(
            prose.matches("fox jumps").count(),
            input.matches("fox jumps").count(),
            "the paragraph must stay one prose region"
        );
        assert_eq!(
            spanned
                .iter()
                .filter(|region| matches!(region.region, Region::Prose(_)))
                .count(),
            1,
            "sentences with no blank line stay one paragraph"
        );
        best = best.min(elapsed);
    }
    best
}

fn assert_linear_step(smaller: Duration, larger: Duration) {
    // Four times the lines. A linear pass lands near four times the time.
    // Twelve times plus 150ms still rejects a full rescan of the tail
    // (about sixteen times) and leaves room for scheduling noise.
    let limit = smaller.as_secs_f64() * 12.0 + 0.15;
    assert!(
        larger.as_secs_f64() < limit,
        "{larger:?} exceeded the linear band {limit:.3}s after {smaller:?}"
    );
}

#[test]
fn long_markdown_paragraph_scales_linearly() {
    let cfg = md_cfg();
    let _ = MarkdownParser.parse_full("Warm the parsers.\n");
    let mut samples = Vec::with_capacity(3);
    for sentences in [2_000usize, 8_000, 32_000] {
        let input = paragraph(sentences);
        let parse_time = time_parse(&input);
        let started = Instant::now();
        let out = format_text(&input, &cfg).expect("format paragraph");
        let format_time = started.elapsed();
        assert_eq!(
            out, input,
            "one sentence per line is already broken; formatting must keep the bytes"
        );
        eprintln!("paragraph sentences={sentences} parse={parse_time:?} format={format_time:?}");
        samples.push(parse_time);
    }
    assert_linear_step(samples[0], samples[1]);
    assert_linear_step(samples[1], samples[2]);
}
