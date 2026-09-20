//! Fixtures for abbreviations, LaTeX math, Org blocks, Markdown fences,
//! and wrapped lists. `sdiff` is empty when only wrapping differs.
//! Default break rules stay unless a fixture requires a change.
//! Both `snapper` and `snapper-fmt` binaries stay.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use snapper_fmt::format::Format;
use snapper_fmt::parser::latex::LatexParser;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::sdiff::sentence_diff;
use snapper_fmt::{FormatConfig, format_text};

fn cfg(format: Format) -> FormatConfig {
    FormatConfig {
        format,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn wrap_cfg(format: Format, max_width: usize) -> FormatConfig {
    FormatConfig {
        format,
        max_width,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn fixture(name: &str) -> String {
    fs::read_to_string(fixture_path(name)).unwrap()
}

fn sdiff_texts(old: &str, new: &str, format: Format, stem: &str) -> String {
    let dir = std::env::temp_dir();
    let old_path = dir.join(format!("snapper_19jf_{stem}_old.txt"));
    let new_path = dir.join(format!("snapper_19jf_{stem}_new.txt"));
    fs::write(&old_path, old).unwrap();
    fs::write(&new_path, new).unwrap();
    let result = sentence_diff(&old_path, &new_path, Some(format), false).unwrap();
    let _ = fs::remove_file(&old_path);
    let _ = fs::remove_file(&new_path);
    result
}

fn assert_sdiff_empty_wrap_only(input: &str, format: Format, stem: &str, wrap_width: usize) {
    let sentence = format_text(input, &cfg(format)).unwrap();
    let wrapped = format_text(input, &wrap_cfg(format, wrap_width)).unwrap();
    let vs_sentence = sdiff_texts(input, &sentence, format, &format!("{stem}_sent"));
    assert!(
        vs_sentence.is_empty(),
        "sdiff must be empty after sentence wrap ({stem}), got:\n{vs_sentence}\n--- input ---\n{input}--- formatted ---\n{sentence}"
    );
    let vs_width = sdiff_texts(input, &wrapped, format, &format!("{stem}_width"));
    assert!(
        vs_width.is_empty(),
        "sdiff must be empty after max_width wrap ({stem}), got:\n{vs_width}\n--- input ---\n{input}--- wrapped ---\n{wrapped}"
    );
}

#[test]
fn abbreviations_do_not_split_titles_or_latin() {
    let input = fixture("abbreviations.txt");
    let out = format_text(&input, &cfg(Format::Plaintext)).unwrap();
    assert!(
        out.contains("Dr. Smith went home.\n"),
        "Dr. must stay in the title sentence, got:\n{out}"
    );
    assert!(
        !out.contains("Dr.\n"),
        "Dr. must not be its own sentence, got:\n{out}"
    );
    assert!(
        out.contains("See Fig. 3 for details.\n"),
        "Fig. must stay with the figure number, got:\n{out}"
    );
    assert!(
        out.contains("Use e.g. snapper here.\n"),
        "e.g. must stay with the example, got:\n{out}"
    );
    assert!(
        out.contains("He was tired.\n"),
        "the sentence after Dr. must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &cfg(Format::Plaintext)).unwrap(), out);
    assert_sdiff_empty_wrap_only(&input, Format::Plaintext, "abbr", 24);
}

#[test]
fn latex_math_stays_atomic_and_following_splits() {
    let input = fixture("math.tex");
    let regions = LatexParser::default().parse(&input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("E = mc^2. not prose")
        )),
        "equation body must stay Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("mc^2. not prose")
        )),
        "equation body must not be Prose, got {regions:?}"
    );
    let out = format_text(&input, &cfg(Format::Latex)).unwrap();
    assert!(
        out.contains("$a. b$"),
        "inline math with an interior period must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("$a.\n"),
        "must not wrap inside inline math, got:\n{out}"
    );
    assert!(
        out.contains("E = mc^2. not prose"),
        "display math must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("See $a. b$ inline.\nNext.\n"),
        "prose around inline math must still split, got:\n{out}"
    );
    assert!(
        out.contains("After the equation.\nNext.\n"),
        "prose after display math must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &cfg(Format::Latex)).unwrap(), out);
    assert_sdiff_empty_wrap_only(&input, Format::Latex, "math", 20);
}

#[test]
fn org_blocks_keep_src_and_split_note() {
    let input = fixture("org_blocks.org");
    let regions = OrgParser.parse(&input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("x = 1. 2")
        )),
        "BEGIN_SRC body must be Code, got {regions:?}"
    );
    let out = format_text(&input, &cfg(Format::Org)).unwrap();
    assert!(
        out.contains("x = 1. 2\nprint(\"stay\")\n"),
        "source block body must not split on interior periods, got:\n{out}"
    );
    assert!(
        out.contains("#+BEGIN_NOTE\nQuoted one.\nQuoted two.\n#+END_NOTE\n"),
        "NOTE fences stay; inner prose must split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext.\n"),
        "prose after the blocks must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &cfg(Format::Org)).unwrap(), out);
    assert_sdiff_empty_wrap_only(&input, Format::Org, "org_blocks", 20);
}

#[test]
fn markdown_fences_stay_and_following_splits() {
    let input = fixture("md_fences.md");
    let regions = MarkdownParser.parse(&input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("x = 1. 2")
        )),
        "fenced body must be Code, got {regions:?}"
    );
    let out = format_text(&input, &cfg(Format::Markdown)).unwrap();
    assert!(
        out.contains("```python\nx = 1. 2\nprint(\"stay\")\n```\n"),
        "fence body must not split on interior periods, got:\n{out}"
    );
    assert!(
        out.contains("Before the fence.\nNext sentence.\n"),
        "prose before the fence must still split, got:\n{out}"
    );
    assert!(
        out.contains("After the fence.\nNext sentence.\n"),
        "prose after the fence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &cfg(Format::Markdown)).unwrap(), out);
    assert_sdiff_empty_wrap_only(&input, Format::Markdown, "md_fences", 20);
}

#[test]
fn wrapped_lists_hang_and_do_not_mint_a_block() {
    let input = fixture("wrapped_lists.md");
    let out = format_text(&input, &cfg(Format::Markdown)).unwrap();
    assert!(
        out.contains("- First item is a long sentence that should wrap at a modest width without minting a new block.\n  Second sentence stays in the item.\n"),
        "list item must hang the second sentence, got:\n{out}"
    );
    assert!(
        out.contains("After the list.\nNext.\n"),
        "prose after the list must still split, got:\n{out}"
    );
    let wrapped = format_text(&input, &wrap_cfg(Format::Markdown, 28)).unwrap();
    assert!(
        wrapped.contains("- First item"),
        "list marker must stay, got:\n{wrapped}"
    );
    let markers = |s: &str| s.lines().filter(|l| l.starts_with("- ")).count();
    assert_eq!(
        markers(&wrapped),
        markers(&input),
        "wrap must not mint a new list marker, got:\n{wrapped}"
    );
    assert!(
        !wrapped.contains("```") && !wrapped.contains("# "),
        "wrap must not mint a fence or heading, got:\n{wrapped}"
    );
    assert_eq!(format_text(&out, &cfg(Format::Markdown)).unwrap(), out);
    assert_sdiff_empty_wrap_only(&input, Format::Markdown, "lists", 28);
}

#[test]
fn default_clause_breaks_stay_off() {
    let default = FormatConfig::default();
    assert!(
        !default.clause_breaks,
        "clause_breaks default must stay false"
    );
    assert_eq!(default.max_width, 0, "max_width default must stay 0");
    let input = "Hello, world; still one sentence.\n";
    let out = format_text(input, &cfg(Format::Plaintext)).unwrap();
    assert_eq!(
        out, input,
        "default break rules must not split on comma or semicolon, got:\n{out}"
    );
}

#[test]
fn snapper_and_snapper_fmt_binaries_exist() {
    let snapper = Path::new(env!("CARGO_BIN_EXE_snapper"));
    let snapper_fmt = snapper.with_file_name("snapper-fmt");
    assert!(
        snapper.exists(),
        "snapper binary missing at {}",
        snapper.display()
    );
    assert!(
        snapper_fmt.exists(),
        "snapper-fmt binary missing at {}",
        snapper_fmt.display()
    );
    for bin in [snapper, snapper_fmt.as_path()] {
        let output = Command::new(bin)
            .arg("--version")
            .output()
            .unwrap_or_else(|e| panic!("{} --version failed: {e}", bin.display()));
        assert!(
            output.status.success(),
            "{} --version status {:?}",
            bin.display(),
            output.status
        );
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(
            text.contains("snapper"),
            "{} --version must name snapper, got {text}",
            bin.display()
        );
    }
}

#[test]
fn init_writes_working_git_hook() {
    let dir = tempfile::tempdir().unwrap();
    let git_init = Command::new("git")
        .args(["-c", "init.templateDir=", "init"])
        .current_dir(dir.path())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("git init");
    assert!(
        git_init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&git_init.stderr)
    );
    fs::write(dir.path().join("note.md"), "One. Two.\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_snapper"))
        .arg("init")
        .current_dir(dir.path())
        .output()
        .expect("snapper init");
    assert!(
        output.status.success(),
        "snapper init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("pre-commit"),
        "init must mention the hook, stderr={stderr}"
    );

    let hooks = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks"])
        .current_dir(dir.path())
        .output()
        .expect("git rev-parse hooks");
    assert!(hooks.status.success());
    let hooks_rel = String::from_utf8_lossy(&hooks.stdout).trim().to_string();
    let hooks_dir = if Path::new(&hooks_rel).is_absolute() {
        PathBuf::from(&hooks_rel)
    } else {
        dir.path().join(&hooks_rel)
    };
    let hook = hooks_dir.join("pre-commit");
    assert!(hook.exists(), "init must write {}", hook.display());
    let hook_text = fs::read_to_string(&hook).unwrap();
    assert!(
        hook_text.contains("Generated by `snapper init`"),
        "hook must be the generated script, got:\n{hook_text}"
    );
    assert!(
        hook_text.contains("--native"),
        "working hook must force native parsers, got:\n{hook_text}"
    );
    assert!(
        hook_text.contains("snapper-fmt"),
        "hook must fall back to snapper-fmt, got:\n{hook_text}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&hook).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "hook must be executable, mode={mode:o}");
    }

    let filter = Command::new("git")
        .args(["config", "--get", "filter.snapper.clean"])
        .current_dir(dir.path())
        .output()
        .expect("git config");
    let clean = String::from_utf8_lossy(&filter.stdout);
    assert!(
        clean.contains("snapper-fmt --native --stdin-filepath %f"),
        "init must configure a working clean filter, got {clean}"
    );

    let bin_dir = Path::new(env!("CARGO_BIN_EXE_snapper"))
        .parent()
        .expect("snapper bin dir");
    let mut paths = vec![bin_dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let path = std::env::join_paths(paths).expect("PATH join");
    let add = Command::new("git")
        .args(["-c", "filter.snapper.clean=cat", "add", "note.md"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(add.success(), "git add note.md must succeed");
    let hook_run = Command::new(&hook)
        .current_dir(dir.path())
        .env("PATH", &path)
        .output()
        .expect("run generated hook");
    assert!(
        hook_run.status.success(),
        "generated hook must run, stderr={} stdout={}",
        String::from_utf8_lossy(&hook_run.stderr),
        String::from_utf8_lossy(&hook_run.stdout)
    );
    let staged = fs::read_to_string(dir.path().join("note.md")).unwrap();
    assert_eq!(
        staged, "One.\nTwo.\n",
        "working hook must format staged prose, got {staged:?}"
    );
}
