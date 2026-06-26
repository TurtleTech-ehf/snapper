use criterion::{Criterion, criterion_group, criterion_main};
use snapper_fmt::format::{Format};
use snapper_fmt::{FormatConfig, format_text};
use std::hint::black_box;

fn short_paragraph(c: &mut Criterion) {
    let input = "Hello world. This is a short test. Another sentence follows here.";
    let cfg = FormatConfig::default();
    c.bench_function("reflow_short_plaintext", |b| {
        b.iter(|| format_text(black_box(input), black_box(&cfg)).unwrap())
    });
}

fn long_paragraph(c: &mut Criterion) {
    let sentence = "The quick brown fox jumps over the lazy dog near the riverbank at dawn. ";
    let input = sentence.repeat(40);
    let cfg = FormatConfig::default();
    c.bench_function("reflow_long_plaintext", |b| {
        b.iter(|| format_text(black_box(&input), black_box(&cfg)).unwrap())
    });
}

fn org_pipeline(c: &mut Criterion) {
    let input = "\
* TODO Title with Vec[T]
Some prose. More prose with *bold. Inside* emphasis.
- List item one. Continuation still.
- Second item.
";
    let cfg = FormatConfig {
        format: Format::Org,
        ..Default::default()
    };
    c.bench_function("reflow_org_sample", |b| {
        b.iter(|| format_text(black_box(input), black_box(&cfg)).unwrap())
    });
}

fn idempotency_double_apply(c: &mut Criterion) {
    let input = "First sentence. Second sentence. Third sentence for width.";
    let cfg = FormatConfig::default();
    c.bench_function("reflow_idempotent_double", |b| {
        b.iter(|| {
            let once = format_text(black_box(input), black_box(&cfg)).unwrap();
            format_text(black_box(&once), black_box(&cfg)).unwrap()
        })
    });
}

criterion_group!(
    benches,
    short_paragraph,
    long_paragraph,
    org_pipeline,
    idempotency_double_apply
);
criterion_main!(benches);
