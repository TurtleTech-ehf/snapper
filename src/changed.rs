//! Diff-aware `--changed`: reflow and check only paragraphs that contain
//! a line git considers added or modified.
//!
//! The comparison is `git diff -U0` against the merge base of `<ref>` and
//! `HEAD` (so `<ref>` itself is `HEAD` when the flag is omitted). Diffs
//! pass `--no-ext-diff` and `:(literal)` pathspecs, and a rename is diffed
//! from both sides so the new path is not reported as a brand-new file.
//! A paragraph is a run of non-blank lines. Editing one line of a
//! paragraph reflows that paragraph, because a sentence break moves the
//! lines around it. Paragraphs with no added or modified line stay byte
//! for byte, including their newline style.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::format::Format;
use crate::{FormatConfig, format_range, format_text};

/// Which lines of a file `--changed` is allowed to rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Touch {
    /// Every line. Used for files that do not exist at the base ref.
    All,
    /// 1-indexed lines added or modified in the work tree.
    Lines(Vec<usize>),
}

/// 1-indexed inclusive line numbers added on the new side of a unified diff.
///
/// Counts come from `@@ -old +new @@` headers. A missing count means one
/// line. A new-side count of zero is a deletion. The paragraph that now
/// borders that hole is still touched, so a deletion-only edit is not skipped.
pub fn added_lines(diff: &str) -> Vec<usize> {
    let mut lines = BTreeSet::new();
    for header in diff.lines().filter_map(hunk_header) {
        let (start, count) = header;
        if count == 0 {
            if start == 0 {
                lines.insert(1);
            } else {
                lines.insert(start);
                if start > 1 {
                    lines.insert(start - 1);
                }
            }
            continue;
        }
        for line in start..start.saturating_add(count) {
            lines.insert(line);
        }
    }
    lines.into_iter().collect()
}

/// Paragraph ranges (1-indexed, inclusive) that contain any `touched` line.
///
/// Blank lines separate paragraphs and are not part of a range. A touched
/// line past the end of `text` is ignored.
pub fn paragraphs_touching(text: &str, touched: &[usize]) -> Vec<(usize, usize)> {
    if touched.is_empty() {
        return Vec::new();
    }
    let touched: BTreeSet<usize> = touched.iter().copied().collect();
    let lines: Vec<&str> = text.lines().collect();
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim().is_empty() {
            index += 1;
            continue;
        }
        let start = index;
        while index < lines.len() && !lines[index].trim().is_empty() {
            index += 1;
        }
        let start_no = start + 1;
        let end_no = index;
        if (start_no..=end_no).any(|line| touched.contains(&line)) {
            ranges.push((start_no, end_no));
        }
    }
    ranges
}

/// Line numbers whose diagnostics count for this touch set.
///
/// `None` means every line. `Some` is every line of a touched paragraph,
/// which is wider than the git hunk when the edit sits inside a paragraph.
pub fn diagnostic_lines(text: &str, touch: &Touch) -> Option<Vec<usize>> {
    match touch {
        Touch::All => None,
        Touch::Lines(lines) => {
            let mut out = Vec::new();
            for (start, end) in paragraphs_touching(text, lines) {
                out.extend(start..=end);
            }
            Some(out)
        }
    }
}

/// Reflow `touch` and leave every other paragraph as it was.
pub fn reflow_touch(input: &str, config: &FormatConfig, touch: &Touch) -> Result<String> {
    match touch {
        Touch::All => format_text(input, config),
        Touch::Lines(lines) if lines.is_empty() => Ok(input.to_string()),
        Touch::Lines(lines) => {
            let ranges = paragraphs_touching(input, lines);
            let mut output = input.to_string();
            // Later ranges first so an earlier range keeps its line numbers.
            for (start, end) in ranges.into_iter().rev() {
                output = format_range(&output, config, start, end)?;
            }
            Ok(output)
        }
    }
}

/// Prose files that differ from `base` or are untracked in the repo at `cwd`.
pub fn changed_prose_files(base: &str) -> Result<Vec<PathBuf>> {
    validate_ref(base)?;
    let top = toplevel(&std::env::current_dir().context("failed to read the working directory")?)?;
    let base = comparison_base(&top, base)?;
    let mut names = BTreeSet::new();
    let diff = git(
        &top,
        [
            "diff",
            "-z",
            "--name-only",
            "--diff-filter=ACMR",
            "--find-renames",
            "--no-ext-diff",
            "--no-color",
            &base,
        ],
    )?;
    names.extend(null_split(&diff));
    let untracked = git(&top, ["ls-files", "-z", "--others", "--exclude-standard"])?;
    names.extend(null_split(&untracked));
    let mut paths = Vec::new();
    for name in names {
        if name.is_empty() {
            continue;
        }
        let path = top.join(&name);
        if path.is_file() && Format::recognized_from_path(&path).is_some() {
            paths.push(path);
        }
    }
    Ok(paths)
}

/// Lines of `path` that `--changed` may rewrite, compared with `base`.
///
/// The file has to live in the repository that contains the current directory.
/// A path in another git checkout, or a path that is not in a checkout at all,
/// is an error. The file's own toplevel is not enough: that would format a
/// foreign repository as if it had been requested here.
pub fn touch_for_path(base: &str, path: &Path) -> Result<Touch> {
    validate_ref(base)?;
    let abs = std::path::absolute(path)
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    let cwd = std::env::current_dir().context("failed to read the current directory")?;
    let cwd_top = toplevel(&cwd)?;
    let file_start = abs.parent().unwrap_or(Path::new("."));
    let top = match toplevel(file_start) {
        Ok(top) if same_directory(&top, &cwd_top) => top,
        Ok(top) => {
            bail!(
                "{} is in a different git repository ({}); --changed only accepts files in {}",
                path.display(),
                top.display(),
                cwd_top.display()
            );
        }
        Err(_) => {
            return Err(RepoPathError::Outside {
                path: path.to_path_buf(),
                top: cwd_top,
            }
            .into());
        }
    };
    // `absolute` does not follow symlinks, so a link inside this checkout can
    // still name a file outside it. The resolved path has to stay under the root.
    ensure_resolved_inside(&abs, &top)?;
    let rel = repo_relative(&abs, &top)?;
    let rel_s = rel.to_string_lossy().replace('\\', "/");
    let base = comparison_base(&top, base)?;
    let raw = git(
        &top,
        [
            "diff",
            "--raw",
            "-z",
            "--find-renames",
            "--no-ext-diff",
            "--no-color",
            &base,
        ],
    )?;
    let specs = pathspecs_for(&parse_raw_z(&raw), &rel_s);
    let mut args = vec![
        "diff".to_string(),
        "-U0".to_string(),
        "--no-color".to_string(),
        "--no-ext-diff".to_string(),
        "--find-renames".to_string(),
        base.clone(),
        "--".to_string(),
    ];
    args.extend(specs);
    let diff = git(&top, args)?;
    if diff_has_file(&diff) {
        if diff.contains("Binary files") && !diff.contains("@@") {
            return Ok(Touch::All);
        }
        return Ok(Touch::Lines(added_lines(&diff)));
    }
    if exists_at_ref(&top, &base, &rel_s)? {
        Ok(Touch::Lines(Vec::new()))
    } else {
        Ok(Touch::All)
    }
}

fn same_directory(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
}

/// Reject a path whose canonical target is not under the work tree.
///
/// `--in-place` must not follow a symlink to a file outside the repository.
/// [`write_inside_repo`] resolves the path and replaces that file, so a link
/// swapped for an outside target between the check and the write is not followed.
pub fn ensure_inside_workdir_repo(path: &Path) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to read the current directory")?;
    let top = toplevel(&cwd)?;
    ensure_resolved_inside(path, &top)
}

/// Write `contents` to the file `path` resolves to, inside the work tree.
///
/// The temp file is renamed onto the canonical path. `rename` replaces a
/// symlink instead of following it, so the write cannot be redirected at a
/// file outside the repository. A hard-linked file is left untouched unless
/// `hardlinks` is [`HardlinkPolicy::Break`], which replaces this path only.
pub fn write_inside_repo(path: &Path, contents: &str, hardlinks: HardlinkPolicy) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to read the current directory")?;
    let top = toplevel(&cwd)?;
    ensure_resolved_inside(path, &top)?;
    let resolved = match path.canonicalize() {
        Ok(resolved) => resolved,
        Err(source) => {
            return Err(RepoPathError::Unresolved {
                path: path.to_path_buf(),
                source,
            }
            .into());
        }
    };
    write_with_policy(&resolved, contents, hardlinks)
}

/// Why a `--changed` path was not accepted.
///
/// [`RepoPathError::is_outside`] is the only case the command skips. A path
/// that cannot be canonicalized is [`RepoPathError::Unresolved`], not an
/// outside-repository warning.
#[derive(Debug, thiserror::Error)]
pub enum RepoPathError {
    /// `path` is not in `top`, or it resolved to somewhere else.
    #[error("{path} is outside the git repository {top}")]
    Outside { path: PathBuf, top: PathBuf },
    /// `path` resolved to `resolved`, which is not under `top`.
    #[error("{path} is outside the git repository {top} (resolved to {resolved})")]
    OutsideResolved {
        path: PathBuf,
        top: PathBuf,
        resolved: PathBuf,
    },
    /// `path` could not be canonicalized. This is not an outside path.
    #[error("failed to resolve {path}")]
    Unresolved {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl RepoPathError {
    /// True when the path is outside this repository, not when it failed to resolve.
    pub fn is_outside(&self) -> bool {
        matches!(self, Self::Outside { .. } | Self::OutsideResolved { .. })
    }
}

/// What to do with a destination that has more than one hard link.
///
/// Overwriting the inode in place cannot be made crash-safe: a killed
/// process or a power loss leaves every name torn, and a backup temp does
/// not survive that. The default is to refuse. [`HardlinkPolicy::Break`]
/// replaces this directory entry only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardlinkPolicy {
    /// Leave the file untouched and return [`HardLinked`].
    Refuse,
    /// Rename a new inode onto this path. Other names keep the old text.
    Break,
}

/// `path` has more than one hard link, and the write was refused.
#[derive(Debug, thiserror::Error)]
#[error(
    "{path} is hard-linked ({links} names); refusing to rewrite it. Pass --break-hardlinks to replace this path only"
)]
pub struct HardLinked {
    /// The path that was left untouched.
    pub path: PathBuf,
    /// `nlink` from the `lstat` that refused the write.
    pub links: u64,
}

/// The hard-link count could not be read, and the write was refused.
#[derive(Debug, thiserror::Error)]
#[error(
    "{path} could not be checked for hard links; refusing to rewrite it. Pass --break-hardlinks to replace this path anyway"
)]
pub struct HardLinkUnverified {
    /// The path that was left untouched.
    pub path: PathBuf,
}

/// `path` was not the same inode when it was opened as when it was statted.
#[derive(Debug, thiserror::Error)]
#[error("{path} changed between stat and open; refusing to write")]
pub struct FileChanged {
    /// The path that was left unwritten.
    pub path: PathBuf,
}

/// Replace `dest` with `contents`, refusing a hard link.
#[cfg(test)]
fn write_replacing(dest: &Path, contents: &str) -> Result<()> {
    write_with_policy(dest, contents, HardlinkPolicy::Refuse)
}

/// Replace `dest` with `contents`.
///
/// On Unix the parent directory is opened once, with `O_DIRECTORY` and
/// `O_NOFOLLOW` on its last component, and every later step uses that
/// directory descriptor: `openat` of the target, the temporary file, and
/// `renameat`. Snapper protects against races on the target's own name and
/// its parent directory entry. It does not defend against an attacker who
/// controls ancestor directories above the parent.
///
/// The temporary file is created with `O_CREAT|O_EXCL|O_NOFOLLOW`. Its
/// create mode is the kept permission bits masked to `0600`, so a `0400`
/// original is not briefly created as `0600`. `fchmod` and `fchown` then
/// run on that descriptor. Setuid, setgid, and sticky are not copied.
///
/// A regular file with more than one hard link is not rewritten in place.
/// [`HardlinkPolicy::Refuse`] returns [`HardLinked`] and does not create a
/// temporary file. [`HardlinkPolicy::Break`] renames the new inode onto this
/// path, so the other names keep the previous inode. A link count that
/// cannot be read is the same refusal ([`HardLinkUnverified`]), unless the
/// policy is [`HardlinkPolicy::Break`].
///
/// A regular file is opened with `O_NOFOLLOW` relative to the parent, and
/// the opened inode's device and inode number must match the earlier
/// `lstat`. The parent descriptor is checked the same way against the
/// `lstat` of the parent path. A mismatch, or an open that fails because
/// the last component became a symlink, refuses the write before any
/// temporary file is created.
///
/// A symlink at `dest` is not followed: `renameat` replaces the directory
/// entry. Its replacement mode is the resolved target's permission bits.
///
/// The new bytes are synced before `renameat`, and the parent descriptor is
/// synced after it. Windows cannot open a directory without
/// `FILE_FLAG_BACKUP_SEMANTICS` and has no directory descriptor here, so
/// that path is best-effort: a private temporary file is renamed by path,
/// and a successful rename is not reported as a failure.
fn write_with_policy(dest: &Path, contents: &str, policy: HardlinkPolicy) -> Result<()> {
    let meta = match std::fs::symlink_metadata(dest) {
        Ok(meta) => Some(meta),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            return Err(err).with_context(|| format!("failed to stat {}", dest.display()));
        }
    };
    if let Some(meta) = meta.as_ref() {
        ensure_hard_links(dest, meta, policy)?;
    }
    #[cfg(unix)]
    {
        write_unix(dest, contents, meta.as_ref())
    }
    #[cfg(not(unix))]
    {
        write_best_effort(dest, contents, meta.as_ref())
    }
}

/// Replace `path` without the `--changed` repository check.
///
/// `--in-place` and `--changed` share this writer. A symlink at `path` is
/// replaced, not followed. A hard link is refused unless `hardlinks` is
/// [`HardlinkPolicy::Break`].
pub fn replace_file(path: &Path, contents: &str, hardlinks: HardlinkPolicy) -> Result<()> {
    write_with_policy(path, contents, hardlinks)
}

/// Permission bits to put on a replacement. Special bits are not copied.
///
/// A symlink's own `lstat` mode is `0777` on Linux and is not a file mode.
/// The resolved target's permission bits are used instead. The directory
/// entry is still replaced rather than followed.
#[cfg(unix)]
fn replacement_mode(dest: &Path, meta: &std::fs::Metadata) -> u32 {
    if meta.file_type().is_symlink() {
        match std::fs::metadata(dest) {
            Ok(target) if target.is_file() => kept_mode(&target),
            _ => 0o600,
        }
    } else {
        kept_mode(meta)
    }
}

#[cfg(unix)]
fn kept_mode(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o777
}

/// Best-effort replacement for targets without a directory descriptor.
///
/// Windows cannot hold the parent open without `FILE_FLAG_BACKUP_SEMANTICS`,
/// so the temporary file, the rename, and the destination are path
/// operations. This does not close the races the Unix directory descriptor
/// closes. A successful rename is not reported as a failure.
#[cfg(not(unix))]
fn write_best_effort(dest: &Path, contents: &str, meta: Option<&std::fs::Metadata>) -> Result<()> {
    let parent = dest
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent = parent.unwrap_or(Path::new("."));
    let mut tmp = private_temp(parent)?;
    write_all_checked(tmp.as_file_mut(), contents.as_bytes())
        .with_context(|| format!("failed to write {}", tmp.path().display()))?;
    if let Some(meta) = meta {
        if !meta.file_type().is_symlink() {
            tmp.as_file()
                .set_permissions(meta.permissions())
                .with_context(|| format!("failed to keep permissions on {}", dest.display()))?;
        }
    }
    sync_file(tmp.as_file()).with_context(|| format!("failed to sync {}", tmp.path().display()))?;
    tmp.persist(dest).map_err(|err| {
        anyhow::Error::from(err.error).context(format!("failed to replace {}", dest.display()))
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn private_temp(parent: &Path) -> Result<tempfile::NamedTempFile> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(".snapper-").suffix(".tmp").rand_bytes(16);
    builder
        .tempfile_in(parent)
        .with_context(|| format!("failed to create a temporary file in {}", parent.display()))
}

fn sync_file(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    SYNC_COUNT.with(|count| count.set(count.get() + 1));
    file.sync_all()
}

#[cfg(test)]
thread_local! {
    static SYNC_COUNT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static WRITE_FAULT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static LINK_QUERY_FAIL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(all(test, unix))]
thread_local! {
    static CHOWN_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static OWNER_WARNINGS: std::cell::RefCell<Vec<String>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static CHOWN_CALLS: std::cell::RefCell<Vec<(Option<u32>, Option<u32>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static BEFORE_PARENT_OPEN: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
    static BEFORE_TARGET_OPEN: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
    static FIXED_TEMP_NAME: std::cell::RefCell<Option<std::ffi::OsString>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(all(test, unix))]
fn reset_sync_count() {
    SYNC_COUNT.with(|count| count.set(0));
}

#[cfg(all(test, unix))]
fn sync_count() -> u32 {
    SYNC_COUNT.with(|count| count.get())
}

#[cfg(all(test, unix))]
fn arm_chown_fault(kind: u8) {
    CHOWN_FAULT.with(|flag| flag.set(kind));
}

#[cfg(all(test, unix))]
fn take_owner_warnings() -> Vec<String> {
    OWNER_WARNINGS.with(|warnings| std::mem::take(&mut *warnings.borrow_mut()))
}

#[cfg(all(test, unix))]
fn take_chown_calls() -> Vec<(Option<u32>, Option<u32>)> {
    CHOWN_CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

/// Test hook between the parent `lstat` and `openat` of that directory.
///
/// The race window is not otherwise reachable from a single thread.
#[cfg(all(test, unix))]
fn arm_before_parent_open(hook: impl FnOnce() + 'static) {
    BEFORE_PARENT_OPEN.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

/// Test hook between the destination `lstat` and `openat` of that name.
///
/// The race window is not otherwise reachable from a single thread.
#[cfg(all(test, unix))]
fn arm_before_target_open(hook: impl FnOnce() + 'static) {
    BEFORE_TARGET_OPEN.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

/// Test hook that pins the temporary name so a planted symlink can match it.
///
/// Production names are random, so a pre-planted path would not collide.
#[cfg(all(test, unix))]
fn arm_fixed_temp_name(name: &str) {
    FIXED_TEMP_NAME.with(|slot| *slot.borrow_mut() = Some(std::ffi::OsString::from(name)));
}

/// Test hook that makes the hard-link query fail once.
#[cfg(all(test, unix))]
fn arm_link_query_failure() {
    LINK_QUERY_FAIL.with(|flag| flag.set(true));
}

#[cfg(unix)]
fn fire_before_parent_open() {
    #[cfg(test)]
    if let Some(hook) = BEFORE_PARENT_OPEN.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
}

#[cfg(unix)]
fn fire_before_target_open() {
    #[cfg(test)]
    if let Some(hook) = BEFORE_TARGET_OPEN.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
}

/// Fail a replacement write after `fail_after` bytes.
///
/// The bytes are written only to the temporary file. The destination is not
/// modified, and the temporary name is removed.
#[cfg(all(test, unix))]
fn arm_write_fault(fail_after: usize) {
    WRITE_FAULT.with(|fault| fault.set(Some(fail_after)));
}

fn write_all_checked(file: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    #[cfg(test)]
    if let Some(fault) = WRITE_FAULT.with(|fault| fault.take()) {
        let n = fault.min(bytes.len());
        if n > 0 {
            file.write_all(&bytes[..n])?;
        }
        return Err(std::io::Error::new(
            std::io::ErrorKind::StorageFull,
            "injected write failure",
        ));
    }
    file.write_all(bytes)
}

/// Open the parent once and perform the replacement relative to that descriptor.
///
/// Snapper protects against races on the target's own name and its parent
/// directory entry. It does not defend against an attacker who controls
/// ancestor directories above the parent. `O_NOFOLLOW` applies only to the
/// last component of the parent path.
#[cfg(unix)]
fn write_unix(dest: &Path, contents: &str, meta: Option<&std::fs::Metadata>) -> Result<()> {
    let parent = dest
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = dest
        .file_name()
        .with_context(|| format!("failed to replace {}", dest.display()))?;
    let parent_meta = std::fs::symlink_metadata(parent)
        .with_context(|| format!("failed to stat {}", parent.display()))?;
    fire_before_parent_open();
    let dir = rustix::fs::openat(
        rustix::fs::CWD,
        parent,
        parent_directory_flags(),
        rustix::fs::Mode::empty(),
    )
    .with_context(|| {
        format!(
            "{} changed between stat and open; refusing to write",
            parent.display()
        )
    })?;
    let dir_stat = rustix::fs::fstat(&dir).with_context(|| {
        format!(
            "{} changed between stat and open; refusing to write",
            parent.display()
        )
    })?;
    if !inode_matches(stat_dev_ino(&dir_stat), device_inode(&parent_meta)) {
        return Err(FileChanged {
            path: parent.to_path_buf(),
        }
        .into());
    }
    if let Some(expected) = meta.filter(|meta| meta.file_type().is_file()) {
        fire_before_target_open();
        let file = rustix::fs::openat(&dir, name, target_open_flags(), rustix::fs::Mode::empty())
            .with_context(|| {
            format!(
                "{} changed between stat and open; refusing to write",
                dest.display()
            )
        })?;
        let opened = rustix::fs::fstat(&file).with_context(|| {
            format!(
                "{} changed between stat and open; refusing to write",
                dest.display()
            )
        })?;
        if !inode_matches(stat_dev_ino(&opened), device_inode(expected)) {
            return Err(FileChanged {
                path: dest.to_path_buf(),
            }
            .into());
        }
    }
    let mode = meta
        .map(|meta| replacement_mode(dest, meta))
        .unwrap_or(0o600);
    let tmp_name = temp_file_name()?;
    let tmp_fd = rustix::fs::openat(&dir, &tmp_name, temp_open_flags(), open_mode(mode & 0o600))
        .with_context(|| format!("failed to create a temporary file in {}", parent.display()))?;
    if let Err(err) = write_temp(tmp_fd, dest, contents, mode, meta) {
        let _ = rustix::fs::unlinkat(&dir, &tmp_name, rustix::fs::AtFlags::empty());
        return Err(err);
    }
    rustix::fs::renameat(&dir, &tmp_name, &dir, name)
        .with_context(|| format!("failed to replace {}", dest.display()))?;
    let dir_file = std::fs::File::from(dir);
    sync_file(&dir_file).with_context(|| format!("failed to sync {}", parent.display()))?;
    Ok(())
}

/// `O_DIRECTORY|O_NOFOLLOW` on the last component of the parent path.
#[cfg(unix)]
fn parent_directory_flags() -> rustix::fs::OFlags {
    rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC
}

/// `O_NOFOLLOW` for the destination name. Removing this flag follows a
/// symlink that still names the inode we already statted.
#[cfg(unix)]
fn target_open_flags() -> rustix::fs::OFlags {
    rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC
}

/// `O_CREAT|O_EXCL|O_NOFOLLOW` for the temporary name. `O_EXCL` refuses an
/// existing name, and `O_NOFOLLOW` refuses to follow one.
#[cfg(unix)]
fn temp_open_flags() -> rustix::fs::OFlags {
    rustix::fs::OFlags::RDWR
        | rustix::fs::OFlags::CREATE
        | rustix::fs::OFlags::EXCL
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC
}

#[cfg(unix)]
fn open_mode(mode: u32) -> rustix::fs::Mode {
    use rustix::fs::Mode;
    let mut bits = Mode::empty();
    for (bit, flag) in [
        (0o400, Mode::RUSR),
        (0o200, Mode::WUSR),
        (0o100, Mode::XUSR),
        (0o040, Mode::RGRP),
        (0o020, Mode::WGRP),
        (0o010, Mode::XGRP),
        (0o004, Mode::ROTH),
        (0o002, Mode::WOTH),
        (0o001, Mode::XOTH),
    ] {
        if mode & bit != 0 {
            bits |= flag;
        }
    }
    bits
}

/// Compare the opened inode with the earlier `lstat`.
#[cfg(unix)]
fn inode_matches(opened: (u64, u64), expected: (u64, u64)) -> bool {
    opened == expected
}

#[cfg(unix)]
fn stat_dev_ino(st: &rustix::fs::Stat) -> (u64, u64) {
    (stat_number(st.st_dev), stat_number(st.st_ino))
}

#[cfg(unix)]
fn stat_number<T>(value: T) -> u64
where
    T: TryInto<u64>,
{
    value.try_into().unwrap_or(0)
}

#[cfg(unix)]
fn temp_file_name() -> Result<std::ffi::OsString> {
    #[cfg(test)]
    if let Some(name) = FIXED_TEMP_NAME.with(|slot| slot.borrow_mut().take()) {
        return Ok(name);
    }
    let mut bytes = [0u8; 16];
    let mut random = std::fs::File::open("/dev/urandom")
        .context("failed to open /dev/urandom for a temporary name")?;
    std::io::Read::read_exact(&mut random, &mut bytes)
        .context("failed to read random bytes for a temporary name")?;
    let mut name = String::from(".snapper-");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        name.push(HEX[(byte >> 4) as usize] as char);
        name.push(HEX[(byte & 0xf) as usize] as char);
    }
    name.push_str(".tmp");
    Ok(std::ffi::OsString::from(name))
}

#[cfg(unix)]
fn write_temp(
    tmp_fd: std::os::fd::OwnedFd,
    dest: &Path,
    contents: &str,
    mode: u32,
    meta: Option<&std::fs::Metadata>,
) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut file = std::fs::File::from(tmp_fd);
    write_all_checked(&mut file, contents.as_bytes())
        .with_context(|| format!("failed to write {}", dest.display()))?;
    file.set_permissions(std::fs::Permissions::from_mode(mode))
        .with_context(|| format!("failed to keep mode {mode:o} on {}", dest.display()))?;
    if let Some(meta) = meta {
        if !meta.file_type().is_symlink() {
            restore_owner(&file, dest, meta)?;
        }
    }
    sync_file(&file).with_context(|| format!("failed to sync {}", dest.display()))?;
    Ok(())
}

fn ensure_hard_links(path: &Path, meta: &std::fs::Metadata, policy: HardlinkPolicy) -> Result<()> {
    if !meta.file_type().is_file() {
        return Ok(());
    }
    match query_links(path, meta) {
        Ok(links) if links > 1 && policy == HardlinkPolicy::Refuse => Err(HardLinked {
            path: path.to_path_buf(),
            links,
        }
        .into()),
        Ok(_) => Ok(()),
        Err(_) if policy == HardlinkPolicy::Break => Ok(()),
        Err(_) => Err(HardLinkUnverified {
            path: path.to_path_buf(),
        }
        .into()),
    }
}

fn query_links(path: &Path, meta: &std::fs::Metadata) -> std::io::Result<u64> {
    #[cfg(test)]
    if LINK_QUERY_FAIL.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "injected link query failure",
        ));
    }
    platform_link_count(path, meta)
}

#[cfg(unix)]
fn platform_link_count(_path: &Path, meta: &std::fs::Metadata) -> std::io::Result<u64> {
    Ok(link_count(meta))
}

#[cfg(windows)]
fn platform_link_count(path: &Path, _meta: &std::fs::Metadata) -> std::io::Result<u64> {
    windows_link_count(path)
}

#[cfg(not(any(unix, windows)))]
fn platform_link_count(_path: &Path, _meta: &std::fs::Metadata) -> std::io::Result<u64> {
    Ok(1)
}

/// `GetFileInformationByHandle` is stable. `MetadataExt::number_of_links`
/// is still unstable on the 1.88 MSRV (`windows_by_handle`).
///
/// An API error is not a link count of one. The caller refuses the write
/// unless `--break-hardlinks` was requested.
#[cfg(windows)]
fn windows_link_count(path: &Path) -> std::io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[repr(C)]
    struct ByHandleFileInformation {
        file_attributes: u32,
        creation_time: FileTime,
        last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }
    unsafe extern "system" {
        fn CreateFileW(
            file_name: *const u16,
            desired_access: u32,
            share_mode: u32,
            security_attributes: *mut core::ffi::c_void,
            creation_disposition: u32,
            flags_and_attributes: u32,
            template_file: isize,
        ) -> isize;
        fn GetFileInformationByHandle(
            file: isize,
            information: *mut ByHandleFileInformation,
        ) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const GENERIC_READ: u32 = 0x8000_0000;
    const FILE_SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
    const OPEN_EXISTING: u32 = 3;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    // SAFETY: `wide` is NUL-terminated UTF-16. The handle is closed before
    // this function returns, and `info` is read only after the query succeeds.
    unsafe {
        let handle = CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_ALL,
            core::ptr::null_mut(),
            OPEN_EXISTING,
            0,
            0,
        );
        if handle == -1 || handle == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut info = std::mem::zeroed::<ByHandleFileInformation>();
        let ok = GetFileInformationByHandle(handle, &mut info);
        let _ = CloseHandle(handle);
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(u64::from(info.number_of_links))
        }
    }
}
#[cfg(unix)]
fn link_count(meta: &std::fs::Metadata) -> u64 {
    #[cfg(target_os = "linux")]
    {
        std::os::linux::fs::MetadataExt::st_nlink(meta)
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::os::unix::fs::MetadataExt::nlink(meta)
    }
}

#[cfg(unix)]
fn device_inode(meta: &std::fs::Metadata) -> (u64, u64) {
    #[cfg(target_os = "linux")]
    {
        use std::os::linux::fs::MetadataExt;
        (meta.st_dev(), meta.st_ino())
    }
    #[cfg(not(target_os = "linux"))]
    {
        use std::os::unix::fs::MetadataExt;
        (meta.dev(), meta.ino())
    }
}

#[cfg(unix)]
fn file_uid(meta: &std::fs::Metadata) -> u32 {
    #[cfg(target_os = "linux")]
    {
        std::os::linux::fs::MetadataExt::st_uid(meta)
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::os::unix::fs::MetadataExt::uid(meta)
    }
}

#[cfg(unix)]
fn file_gid(meta: &std::fs::Metadata) -> u32 {
    #[cfg(target_os = "linux")]
    {
        std::os::linux::fs::MetadataExt::st_gid(meta)
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::os::unix::fs::MetadataExt::gid(meta)
    }
}

/// Put the original owner back on `file` when this process is allowed to.
///
/// uid and gid are tried together. If that is refused, the gid is tried
/// alone. Anything that cannot be kept is warned and the write still stands.
#[cfg(unix)]
fn restore_owner(file: &std::fs::File, path: &Path, meta: &std::fs::Metadata) -> Result<()> {
    let uid = file_uid(meta);
    let gid = file_gid(meta);
    let euid = current_euid();
    if euid != 0 && euid != uid {
        warn_owner(
            path,
            &format!("could not preserve owner {uid} or group {gid}"),
        );
        return Ok(());
    }
    match fchown_fd(file, Some(uid), Some(gid)) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            match fchown_fd(file, None, Some(gid)) {
                Ok(()) => {
                    warn_owner(
                        path,
                        &format!("could not preserve uid {uid}; kept group {gid}"),
                    );
                    Ok(())
                }
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                    warn_owner(
                        path,
                        &format!("could not preserve owner {uid} or group {gid}"),
                    );
                    Ok(())
                }
                Err(err) => Err(err)
                    .with_context(|| format!("failed to keep the owner of {}", path.display())),
            }
        }
        Err(err) => {
            Err(err).with_context(|| format!("failed to keep the owner of {}", path.display()))
        }
    }
}

#[cfg(unix)]
fn fchown_fd(file: &std::fs::File, uid: Option<u32>, gid: Option<u32>) -> std::io::Result<()> {
    #[cfg(test)]
    CHOWN_CALLS.with(|calls| calls.borrow_mut().push((uid, gid)));
    #[cfg(test)]
    {
        let fault = CHOWN_FAULT.with(|flag| flag.get());
        if fault == 2 || (fault == 1 && uid.is_some()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected chown failure",
            ));
        }
    }
    std::os::unix::fs::fchown(file, uid, gid)
}

#[cfg(unix)]
fn warn_owner(path: &Path, detail: &str) {
    eprintln!("warning: {detail} ({})", path.display());
    #[cfg(test)]
    OWNER_WARNINGS.with(|warnings| warnings.borrow_mut().push(detail.to_string()));
}

#[cfg(unix)]
fn current_euid() -> u32 {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    unsafe { geteuid() }
}

fn ensure_resolved_inside(path: &Path, top: &Path) -> Result<()> {
    let top_c = top.canonicalize().unwrap_or_else(|_| top.to_path_buf());
    let resolved = match path.canonicalize() {
        Ok(resolved) => resolved,
        Err(source) => {
            return Err(RepoPathError::Unresolved {
                path: path.to_path_buf(),
                source,
            }
            .into());
        }
    };
    if resolved.starts_with(&top_c) {
        return Ok(());
    }
    Err(RepoPathError::OutsideResolved {
        path: path.to_path_buf(),
        top: top.to_path_buf(),
        resolved,
    }
    .into())
}

fn repo_relative(path: &Path, top: &Path) -> Result<PathBuf> {
    ensure_resolved_inside(path, top)?;
    if let Ok(rel) = path.strip_prefix(top) {
        return Ok(rel.to_path_buf());
    }
    let path_c = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let top_c = top.canonicalize().unwrap_or_else(|_| top.to_path_buf());
    Ok(path_c
        .strip_prefix(&top_c)
        .map(|rel| rel.to_path_buf())
        .map_err(|_| RepoPathError::Outside {
            path: path.to_path_buf(),
            top: top.to_path_buf(),
        })?)
}

fn validate_ref(base: &str) -> Result<()> {
    if base.is_empty()
        || base.starts_with('-')
        || base
            .chars()
            .any(|c| c.is_whitespace() || c == '\0' || c == ':')
    {
        bail!("invalid git ref {base:?} for --changed");
    }
    Ok(())
}

fn toplevel(start: &Path) -> Result<PathBuf> {
    let stdout = git(start, ["rev-parse", "--show-toplevel"]).with_context(|| {
        format!(
            "not a git repository (needed for --changed; looked from {})",
            start.display()
        )
    })?;
    let line = stdout
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .context("git rev-parse --show-toplevel returned no path")?;
    Ok(PathBuf::from(line))
}

fn exists_at_ref(top: &Path, base: &str, rel: &str) -> Result<bool> {
    let spec = format!("{base}:{rel}");
    let output = Command::new("git")
        .arg("-C")
        .arg(top)
        .args(["cat-file", "-e", &spec])
        .output()
        .with_context(|| format!("failed to run git cat-file -e {spec}"))?;
    Ok(output.status.success())
}

/// Commit `git diff` should use: the merge base of `requested` and `HEAD`.
///
/// `merge-base HEAD HEAD` is `HEAD`, so the default ref still diffs the
/// work tree against the current commit. A branch that has not touched a
/// file does not inherit commits that landed only on the other ref.
fn comparison_base(top: &Path, requested: &str) -> Result<String> {
    if !head_exists(top) {
        if requested != "HEAD" {
            bail!("--changed-from {requested} needs a commit, and this repository has no HEAD yet");
        }
        return empty_tree(top);
    }
    let stdout = git(top, ["merge-base", requested, "HEAD"])?;
    let base = stdout.trim();
    if base.is_empty() || base.chars().any(|c| c.is_whitespace() || c == '\0') {
        bail!("git merge-base {requested} HEAD returned no commit");
    }
    Ok(base.to_string())
}

fn head_exists(top: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(top)
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// The empty tree. `git diff` against it lists every tracked path, and an
/// unborn `HEAD` still has untracked prose to check.
fn empty_tree(top: &Path) -> Result<String> {
    let stdout = git(top, ["hash-object", "-t", "tree", "/dev/null"])?;
    let hash = stdout.trim();
    if hash.is_empty() {
        bail!("git hash-object did not return the empty tree");
    }
    Ok(hash.to_string())
}

struct RawChange {
    old: String,
    new: String,
}

/// `git diff --raw -z` records. A rename or copy is `meta\0old\0new\0`.
/// Every other status is `meta\0path\0`.
fn parse_raw_z(data: &str) -> Vec<RawChange> {
    let parts: Vec<&str> = data.split('\0').collect();
    let mut index = 0;
    let mut out = Vec::new();
    while index < parts.len() {
        let meta = parts[index];
        if meta.is_empty() {
            index += 1;
            continue;
        }
        if !meta.starts_with(':') {
            break;
        }
        let status = meta.split_whitespace().next_back().unwrap_or("");
        let code = status.chars().next().unwrap_or('?');
        if code == 'R' || code == 'C' {
            if index + 2 >= parts.len() {
                break;
            }
            out.push(RawChange {
                old: parts[index + 1].to_string(),
                new: parts[index + 2].to_string(),
            });
            index += 3;
        } else if index + 1 < parts.len() {
            let path = parts[index + 1].to_string();
            out.push(RawChange {
                old: path.clone(),
                new: path,
            });
            index += 2;
        } else {
            break;
        }
    }
    out
}

/// Pathspecs for `rel`. A rename is diffed from both names: the new path
/// alone is a new file, so every line looks added.
fn pathspecs_for(changes: &[RawChange], rel: &str) -> Vec<String> {
    if let Some(change) = changes
        .iter()
        .find(|change| change.new == rel || change.old == rel)
    {
        if change.old != change.new {
            return vec![literal_pathspec(&change.old), literal_pathspec(&change.new)];
        }
    }
    vec![literal_pathspec(rel)]
}

/// `note[1].md` is a character class to git unless the pathspec is literal.
fn literal_pathspec(path: &str) -> String {
    format!(":(literal){path}")
}

fn git<I, S>(dir: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let stored: Vec<String> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect();
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(&stored)
        .output()
        .with_context(|| format!("failed to run git {}", stored.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", stored.join(" "), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn null_split(text: &str) -> Vec<String> {
    text.split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn diff_has_file(diff: &str) -> bool {
    diff.lines().any(|line| line.starts_with("diff --git "))
}

/// New-side `(start, count)` from a hunk header, if `line` is one.
fn hunk_header(line: &str) -> Option<(usize, usize)> {
    let rest = line.strip_prefix("@@ ")?;
    let header = rest.split(" @@").next()?;
    let plus = header
        .split_whitespace()
        .find(|part| part.starts_with('+'))?;
    let spec = plus.strip_prefix('+')?;
    let (start, count) = match spec.split_once(',') {
        Some((start, count)) => (start.parse().ok()?, count.parse().ok()?),
        None => (spec.parse().ok()?, 1),
    };
    Some((start, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunk_headers_collect_the_new_side() {
        let diff = "\
diff --git a/paper.md b/paper.md
index 111..222 100644
--- a/paper.md
+++ b/paper.md
@@ -1,2 +1,3 @@
+added
@@ -10 +12,2 @@
@@ -20,1 +22,0 @@
@@ -30 +30 @@
";
        assert_eq!(added_lines(diff), vec![1, 2, 3, 12, 13, 21, 22, 30]);
    }

    #[test]
    fn a_deletion_hunk_touches_the_bordering_line() {
        let diff = "@@ -4 +3,0 @@ Fused sentence. Still fused.\n-Trailing line.\n";
        assert_eq!(added_lines(diff), vec![2, 3]);
        let at_start = "@@ -1 +0,0 @@\n-Intro line.\n";
        assert_eq!(added_lines(at_start), vec![1]);
    }

    #[test]
    fn a_new_file_hunk_starts_at_one() {
        let diff = "@@ -0,0 +1,2 @@\n+one\n+two\n";
        assert_eq!(added_lines(diff), vec![1, 2]);
    }

    #[test]
    fn paragraphs_skip_blanks_and_untouched_blocks() {
        let text = "One. Two.\n\nKeep this.\nStill this.\n\nTail.\n";
        assert_eq!(paragraphs_touching(text, &[1]), vec![(1, 1)]);
        assert_eq!(paragraphs_touching(text, &[4]), vec![(3, 4)]);
        assert_eq!(paragraphs_touching(text, &[]), Vec::<(usize, usize)>::new());
        assert_eq!(paragraphs_touching(text, &[3, 6]), vec![(3, 4), (6, 6)]);
    }

    #[test]
    fn diagnostic_lines_cover_the_paragraph() {
        let text = "One. Two.\nMore.\n\nOther.\n";
        let touch = Touch::Lines(vec![2]);
        assert_eq!(diagnostic_lines(text, &touch), Some(vec![1, 2]));
        assert_eq!(diagnostic_lines(text, &Touch::All), None);
        assert_eq!(
            diagnostic_lines(text, &Touch::Lines(vec![])),
            Some(Vec::<usize>::new())
        );
    }

    #[test]
    fn a_ref_that_looks_like_a_flag_is_rejected() {
        assert!(validate_ref("--cached").is_err());
        assert!(validate_ref("origin/main").is_ok());
        assert!(validate_ref("HEAD~1").is_ok());
        assert!(validate_ref("HEAD:file").is_err());
    }

    #[test]
    fn reflow_touch_leaves_an_untouched_paragraph() {
        let input = "Old fused sentence. Still old.\n\nFresh sentence.\n";
        let config = crate::FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let kept = reflow_touch(input, &config, &Touch::Lines(vec![3])).unwrap();
        assert_eq!(kept, input);
        let fixed = reflow_touch(input, &config, &Touch::Lines(vec![1])).unwrap();
        assert_eq!(
            fixed,
            "Old fused sentence.\nStill old.\n\nFresh sentence.\n"
        );
    }

    #[test]
    fn reflow_touch_keeps_crlf_on_a_partial_edit() {
        let input = "Old fused sentence. Still old.\r\n\r\nFresh sentence. Now fused.\r\n";
        let config = crate::FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let fixed = reflow_touch(input, &config, &Touch::Lines(vec![3])).unwrap();
        assert_eq!(
            fixed,
            "Old fused sentence. Still old.\r\n\r\nFresh sentence.\r\nNow fused.\r\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn replacing_a_symlink_does_not_follow_it() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.md");
        let link = dir.path().join("notes.md");
        std::fs::write(&outside, "secret\n").unwrap();
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        write_replacing(&link, "pwned\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&outside).unwrap(),
            "secret\n",
            "rename must not follow the symlink into the outside file"
        );
        assert!(
            !std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the path must be replaced with a regular file"
        );
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "pwned\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_preplanted_temp_symlink_is_not_followed() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        std::fs::write(&dest, "Two sentences. Here.\n").unwrap();
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600)).unwrap();
        let outside = dir.path().join("outside.md");
        std::fs::write(&outside, "secret\n").unwrap();
        let planted = dir
            .path()
            .join(format!(".snapper-{}-notes.md.tmp", std::process::id()));
        std::os::unix::fs::symlink(&outside, &planted).unwrap();
        write_replacing(&dest, "Two sentences.\nHere.\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&outside).unwrap(),
            "secret\n",
            "a symlink at the old temp name must not be followed"
        );
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            "Two sentences.\nHere.\n"
        );
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "mode must stay 0600, got {mode:04o}");
    }

    #[cfg(unix)]
    #[test]
    fn replacement_mode_is_never_wider_than_the_original() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("secret.md");
        std::fs::write(&dest, "x\n").unwrap();
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o400)).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let wider = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(AtomicU32::new(0));
        let dir_path = dir.path().to_path_buf();
        let stop_w = Arc::clone(&stop);
        let wider_w = Arc::clone(&wider);
        let seen_w = Arc::clone(&seen);
        let watcher = std::thread::spawn(move || {
            while !stop_w.load(Ordering::Relaxed) {
                let Ok(entries) = std::fs::read_dir(&dir_path) else {
                    continue;
                };
                for entry in entries.flatten() {
                    if entry.file_name() == "secret.md" || entry.file_name() == "payload" {
                        continue;
                    }
                    let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                        continue;
                    };
                    if meta.file_type().is_symlink() {
                        wider_w.store(true, Ordering::Relaxed);
                        continue;
                    }
                    let mode = meta.permissions().mode() & 0o777;
                    seen_w.fetch_max(mode, Ordering::Relaxed);
                    if mode & !0o400 != 0 {
                        wider_w.store(true, Ordering::Relaxed);
                    }
                }
            }
        });
        let payload = "Sentence stands here. Another follows it.\n".repeat(400_000);
        write_replacing(&dest, &payload).unwrap();
        stop.store(true, Ordering::Relaxed);
        watcher.join().unwrap();
        let final_mode = std::fs::metadata(&dest).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            final_mode, 0o400,
            "final mode must stay 0400, got {final_mode:04o}"
        );
        assert!(
            !wider.load(Ordering::Relaxed),
            "a temp file was wider than 0400 (saw {:04o})",
            seen.load(Ordering::Relaxed)
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_hard_link_is_refused_and_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("a.md");
        let second = dir.path().join("b.md");
        let original = "ORIGINAL-BYTES-stay\n";
        std::fs::write(&first, original).unwrap();
        std::fs::hard_link(&first, &second).unwrap();
        let err = write_replacing(&first, "REPLACED-BYTES-now\n").unwrap_err();
        let text = format!("{err:#}");
        assert!(
            text.contains("hard-linked") && text.contains("a.md"),
            "the error must name the hard-linked file, got {text}"
        );
        assert!(
            err.downcast_ref::<HardLinked>().is_some()
                || err
                    .chain()
                    .any(|cause| cause.downcast_ref::<HardLinked>().is_some()),
            "the refusal must be a typed hard-link error, got {text}"
        );
        assert_eq!(std::fs::read_to_string(&first).unwrap(), original);
        assert_eq!(std::fs::read_to_string(&second).unwrap(), original);
        assert_eq!(link_count(&std::fs::metadata(&first).unwrap()), 2);
        assert!(
            snapper_temps(dir.path()).is_empty(),
            "refusing a hard link must not leave temp files: {:?}",
            snapper_temps(dir.path())
        );
    }

    #[cfg(unix)]
    #[test]
    fn break_hardlinks_replaces_this_name_only() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("a.md");
        let second = dir.path().join("b.md");
        let original = "ORIGINAL-BYTES-stay\n";
        let replaced = "REPLACED-BYTES-now\n";
        std::fs::write(&first, original).unwrap();
        std::fs::hard_link(&first, &second).unwrap();
        write_with_policy(&first, replaced, HardlinkPolicy::Break).unwrap();
        assert_eq!(std::fs::read_to_string(&first).unwrap(), replaced);
        assert_eq!(
            std::fs::read_to_string(&second).unwrap(),
            original,
            "the other name must keep the old inode"
        );
        assert_eq!(link_count(&std::fs::metadata(&first).unwrap()), 1);
        assert_eq!(link_count(&std::fs::metadata(&second).unwrap()), 1);
    }

    #[cfg(unix)]
    #[test]
    fn special_mode_bits_are_stripped_and_the_group_is_kept() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("script.md");
        std::fs::write(&dest, "Two sentences. Here.\n").unwrap();
        std::os::unix::fs::chown(&dest, None, Some(4)).unwrap();
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o7755)).unwrap();
        let before = std::fs::metadata(&dest).unwrap();
        let uid = file_uid(&before);
        let gid = file_gid(&before);
        write_replacing(&dest, "Two sentences.\nHere.\n").unwrap();
        let after = std::fs::metadata(&dest).unwrap();
        let mode = after.permissions().mode() & 0o7777;
        assert_eq!(
            mode, 0o755,
            "setuid, setgid, and sticky must be cleared, got {mode:04o}"
        );
        assert_eq!(file_uid(&after), uid, "uid must stay {uid}");
        assert_eq!(
            file_gid(&after),
            gid,
            "gid must stay {gid}, got {}",
            file_gid(&after)
        );
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            "Two sentences.\nHere.\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_hard_link_is_left_untouched() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("a.md");
        let second = dir.path().join("b.md");
        let original = "ORIGINAL-BYTES-stay\n";
        std::fs::write(&first, original).unwrap();
        std::fs::hard_link(&first, &second).unwrap();
        std::fs::set_permissions(&first, std::fs::Permissions::from_mode(0o400)).unwrap();
        let err = write_replacing(&first, "REPLACED-BYTES-now\n").unwrap_err();
        let text = format!("{err:#}");
        assert!(
            text.contains("hard-linked"),
            "a 0400 hard link must be refused, got {text}"
        );
        assert_eq!(std::fs::read_to_string(&first).unwrap(), original);
        assert_eq!(std::fs::read_to_string(&second).unwrap(), original);
        let mode = std::fs::metadata(&first).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o400, "mode must stay 0400, got {mode:04o}");
        assert_eq!(link_count(&std::fs::metadata(&first).unwrap()), 2);
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_write_leaves_the_original_and_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        let original = "ORIGINAL-BYTES-stay\n";
        std::fs::write(&dest, original).unwrap();
        arm_write_fault(4);
        let err = write_replacing(&dest, "REPLACED-BYTES-now\n").unwrap_err();
        let text = format!("{err:#}");
        assert!(
            text.contains("injected write failure"),
            "the injected failure must surface, got {text}"
        );
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            original,
            "a failed write must leave the original bytes, which differ from the new prefix"
        );
        assert!(
            snapper_temps(dir.path()).is_empty(),
            "a failed write must not leave temp files: {:?}",
            snapper_temps(dir.path())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_swapped_in_before_open_is_not_written() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        let victim = dir.path().join("victim.md");
        std::fs::write(&dest, "ORIGINAL-BYTES-stay\n").unwrap();
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o640)).unwrap();
        std::fs::write(&victim, "victim-secret-bytes\n").unwrap();
        std::fs::set_permissions(&victim, std::fs::Permissions::from_mode(0o604)).unwrap();
        let swapped = dest.clone();
        let target = victim.clone();
        arm_before_target_open(move || {
            let _ = std::fs::remove_file(&swapped);
            std::os::unix::fs::symlink(&target, &swapped).unwrap();
        });
        let err = write_replacing(&dest, "REPLACED-BYTES-now\n").unwrap_err();
        let text = format!("{err:#}");
        assert!(
            text.contains("refusing to write") || text.contains("changed between stat and open"),
            "a symlink swapped in before open must be refused, got {text}"
        );
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "victim-secret-bytes\n"
        );
        let mode = std::fs::symlink_metadata(&victim)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o604,
            "the symlink target must keep its mode, got {mode:04o}"
        );
        assert!(
            std::fs::symlink_metadata(&dest)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the raced symlink must stay a symlink"
        );
        assert!(
            snapper_temps(dir.path()).is_empty(),
            "a refused write must not leave temp files: {:?}",
            snapper_temps(dir.path())
        );
    }

    #[cfg(unix)]
    #[test]
    fn replacing_a_symlink_uses_the_target_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.md");
        let link = dir.path().join("notes.md");
        std::fs::write(&outside, "secret\n").unwrap();
        std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o640)).unwrap();
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        write_replacing(&link, "pwned\n").unwrap();
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "secret\n");
        let mode = std::fs::metadata(&link).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o640,
            "the replacement must take the target mode, not the symlink's 0777, got {mode:04o}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn replacing_a_file_syncs_it_and_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        std::fs::write(&dest, "Two sentences. Here.\n").unwrap();
        reset_sync_count();
        write_replacing(&dest, "Two sentences.\nHere.\n").unwrap();
        assert!(
            sync_count() >= 2,
            "the temp file and its directory must both be synced, saw {}",
            sync_count()
        );
    }

    /// The replacement name is a symlink to the same inode, so `fstat`
    /// would match the earlier `lstat`. Only `O_NOFOLLOW` refuses the open.
    ///
    /// The hook is required: the swap has to land between `lstat` and `openat`.
    #[cfg(unix)]
    #[test]
    fn a_symlink_to_the_same_inode_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        let keeper = dir.path().join("keeper.md");
        let original = "ORIGINAL-BYTES-stay\n";
        std::fs::write(&dest, original).unwrap();
        let swapped = dest.clone();
        let kept = keeper.clone();
        arm_before_target_open(move || {
            std::fs::hard_link(&swapped, &kept).unwrap();
            std::fs::remove_file(&swapped).unwrap();
            std::os::unix::fs::symlink("keeper.md", &swapped).unwrap();
        });
        let result = write_replacing(&dest, "REPLACED-BYTES-now\n");
        let Err(err) = result else {
            panic!(
                "O_NOFOLLOW was not required: the write succeeded, dest symlink={} contents={:?}",
                std::fs::symlink_metadata(&dest)
                    .unwrap()
                    .file_type()
                    .is_symlink(),
                std::fs::read_to_string(&dest).ok()
            );
        };
        let text = format!("{err:#}");
        assert!(
            text.contains("refusing to write"),
            "O_NOFOLLOW must refuse a same-inode symlink, got {text}"
        );
        assert!(
            std::fs::symlink_metadata(&dest)
                .unwrap()
                .file_type()
                .is_symlink(),
            "following the symlink and renaming over it must fail this test"
        );
        assert_eq!(std::fs::read_to_string(&keeper).unwrap(), original);
        assert!(snapper_temps(dir.path()).is_empty());
    }

    /// Another regular file occupies the name. There is no symlink, so
    /// `O_NOFOLLOW` succeeds and only the device/inode compare refuses.
    ///
    /// The hook is required: the swap has to land between `lstat` and `openat`.
    #[cfg(unix)]
    #[test]
    fn a_regular_file_swapped_for_another_regular_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        let original = "ORIGINAL-BYTES-stay\n";
        let victim = "VICTIM-BYTES-other\n";
        std::fs::write(&dest, original).unwrap();
        let swapped = dest.clone();
        let aside = dir.path().join("notes.md.old");
        let moved = aside.clone();
        arm_before_target_open(move || {
            std::fs::rename(&swapped, &moved).unwrap();
            std::fs::write(&swapped, victim).unwrap();
        });
        let result = write_replacing(&dest, "REPLACED-BYTES-now\n");
        let Err(err) = result else {
            panic!(
                "the dev/ino check was not required: the write succeeded, dest={:?} aside={:?}",
                std::fs::read_to_string(&dest).ok(),
                std::fs::read_to_string(&aside).ok()
            );
        };
        let text = format!("{err:#}");
        assert!(
            text.contains("changed between stat and open"),
            "a different regular file must fail the dev/ino check, got {text}"
        );
        assert!(
            err.downcast_ref::<FileChanged>().is_some()
                || err
                    .chain()
                    .any(|cause| cause.downcast_ref::<FileChanged>().is_some()),
            "the mismatch must be a typed file-changed error, got {text}"
        );
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            victim,
            "the swapped-in file must be left unwritten"
        );
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), original);
        assert!(
            !std::fs::symlink_metadata(&dest)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(snapper_temps(dir.path()).is_empty());
    }

    /// The parent directory entry becomes a symlink to the same directory.
    /// The directory inode matches, so only `O_NOFOLLOW` on the parent open
    /// refuses. A symlink to a different directory would also fail the
    /// directory `fstat` and would not isolate this flag.
    ///
    /// The hook is required: the swap has to land between `lstat` and `openat`.
    #[cfg(unix)]
    #[test]
    fn a_parent_swapped_for_a_symlink_to_the_same_directory_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let sub = root.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let notes = sub.join("notes.md");
        let original = "ORIGINAL-BYTES-stay\n";
        std::fs::write(&notes, original).unwrap();
        let parent = sub.clone();
        let moved = root.path().join("sub.real");
        arm_before_parent_open(move || {
            std::fs::rename(&parent, &moved).unwrap();
            std::os::unix::fs::symlink("sub.real", &parent).unwrap();
        });
        let kept = root.path().join("sub.real").join("notes.md");
        let result = write_replacing(&notes, "REPLACED-BYTES-now\n");
        let Err(err) = result else {
            panic!(
                "parent O_NOFOLLOW was not required: the write succeeded, kept={:?}",
                std::fs::read_to_string(&kept).ok()
            );
        };
        let text = format!("{err:#}");
        assert!(
            text.contains("refusing to write"),
            "O_NOFOLLOW on the parent must refuse the symlink, got {text}"
        );
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), original);
        assert!(
            std::fs::symlink_metadata(root.path().join("sub"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    /// The parent path is a new directory that still holds the same file
    /// inode. `O_NOFOLLOW` succeeds. Only the parent descriptor's device
    /// and inode compare refuses.
    ///
    /// The hook is required: the swap has to land between `lstat` and `openat`.
    #[cfg(unix)]
    #[test]
    fn a_parent_replaced_by_another_directory_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let sub = root.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let notes = sub.join("notes.md");
        let original = "ORIGINAL-BYTES-stay\n";
        std::fs::write(&notes, original).unwrap();
        let parent = sub.clone();
        let moved = root.path().join("sub.old");
        arm_before_parent_open(move || {
            std::fs::rename(&parent, &moved).unwrap();
            std::fs::create_dir(&parent).unwrap();
            std::fs::hard_link(moved.join("notes.md"), parent.join("notes.md")).unwrap();
        });
        let result = write_replacing(&notes, "REPLACED-BYTES-now\n");
        let Err(err) = result else {
            panic!(
                "the parent dev/ino check was not required: the write succeeded, new={:?} old={:?}",
                std::fs::read_to_string(root.path().join("sub").join("notes.md")).ok(),
                std::fs::read_to_string(root.path().join("sub.old").join("notes.md")).ok()
            );
        };
        let text = format!("{err:#}");
        assert!(
            text.contains("changed between stat and open"),
            "the parent dirfd must not match a replacement directory, got {text}"
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("sub.old").join("notes.md")).unwrap(),
            original
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("sub").join("notes.md")).unwrap(),
            original,
            "renaming onto the new directory must fail this test"
        );
    }

    /// The exact temporary name is planted as a symlink. `O_EXCL` refuses
    /// an existing name and `O_NOFOLLOW` refuses to follow it. Either flag
    /// alone leaves the victim untouched; the mutation that follows the
    /// symlink removes both.
    ///
    /// The fixed name is a test hook. Production names are random.
    #[cfg(unix)]
    #[test]
    fn a_preplanted_temp_name_symlink_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        let original = "ORIGINAL-BYTES-stay\n";
        std::fs::write(&dest, original).unwrap();
        let outside = dir.path().join("outside.md");
        std::fs::write(&outside, "secret\n").unwrap();
        let planted = dir.path().join(".snapper-fixed.tmp");
        std::os::unix::fs::symlink(&outside, &planted).unwrap();
        arm_fixed_temp_name(".snapper-fixed.tmp");
        let result = write_replacing(&dest, "REPLACED-BYTES-now\n");
        let Err(err) = result else {
            panic!(
                "the temp open followed the planted symlink: dest={:?} outside={:?}",
                std::fs::read_to_string(&dest).ok(),
                std::fs::read_to_string(&outside).ok()
            );
        };
        let text = format!("{err:#}");
        assert!(
            text.contains("temporary file"),
            "creating the planted name must fail, got {text}"
        );
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "secret\n");
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), original);
        assert!(
            std::fs::symlink_metadata(&planted)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_unverifiable_link_count_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        let original = "ORIGINAL-BYTES-stay\n";
        std::fs::write(&dest, original).unwrap();
        arm_link_query_failure();
        let result = write_replacing(&dest, "REPLACED-BYTES-now\n");
        let Err(err) = result else {
            panic!(
                "a link-query error was treated as a single link, dest={:?}",
                std::fs::read_to_string(&dest).ok()
            );
        };
        let text = format!("{err:#}");
        assert!(
            text.contains("could not be checked"),
            "a link-query error must be refused, got {text}"
        );
        assert!(
            err.downcast_ref::<HardLinkUnverified>().is_some()
                || err
                    .chain()
                    .any(|cause| cause.downcast_ref::<HardLinkUnverified>().is_some()),
            "the refusal must be typed, got {text}"
        );
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), original);
        assert!(snapper_temps(dir.path()).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn an_unverifiable_link_count_may_break() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        std::fs::write(&dest, "ORIGINAL-BYTES-stay\n").unwrap();
        arm_link_query_failure();
        write_with_policy(&dest, "REPLACED-BYTES-now\n", HardlinkPolicy::Break).unwrap();
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            "REPLACED-BYTES-now\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_refused_uid_still_tries_the_group() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        std::fs::write(&dest, "Two sentences. Here.\n").unwrap();
        let gid = file_gid(&std::fs::metadata(&dest).unwrap());
        let _ = take_owner_warnings();
        let _ = take_chown_calls();
        arm_chown_fault(1);
        write_replacing(&dest, "Two sentences.\nHere.\n")
            .expect("a refused uid must fall back to the gid and still replace the file");
        arm_chown_fault(0);
        let calls = take_chown_calls();
        assert!(
            calls
                .iter()
                .any(|(uid, group)| uid.is_none() && *group == Some(gid)),
            "removing the gid-only fchown must fail this test, got {calls:?}"
        );
        let warnings = take_owner_warnings();
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("kept group")),
            "the warning must say the group was kept, got {warnings:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_unpreservable_owner_is_warned() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        std::fs::write(&dest, "Two sentences. Here.\n").unwrap();
        let _ = take_owner_warnings();
        arm_chown_fault(2);
        write_replacing(&dest, "Two sentences.\nHere.\n").unwrap();
        arm_chown_fault(0);
        let warnings = take_owner_warnings();
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("could not preserve")),
            "a group that cannot be kept must be warned, got {warnings:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            "Two sentences.\nHere.\n"
        );
    }

    #[cfg(unix)]
    fn snapper_temps(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".snapper-"))
            .collect()
    }

    #[cfg(not(unix))]
    #[test]
    fn replacing_a_file_does_not_fail_after_the_rename() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.md");
        std::fs::write(&dest, "Two sentences. Here.\n").unwrap();
        write_replacing(&dest, "Two sentences.\nHere.\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            "Two sentences.\nHere.\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_is_not_outside_the_repository() {
        let root = std::env::current_dir().unwrap();
        let dir = tempfile::tempdir_in(&root).unwrap();
        let link = dir.path().join("missing.md");
        std::os::unix::fs::symlink("no-such-target.md", &link).unwrap();
        let err = touch_for_path("HEAD", &link).unwrap_err();
        let text = format!("{err:#}");
        assert!(
            !text.contains("outside the git repository"),
            "a canonicalize failure must not be reported as outside, got {text}"
        );
        assert!(
            text.contains("failed to resolve"),
            "a dangling symlink must be a resolve error, got {text}"
        );
        assert!(
            err.chain().any(|cause| {
                cause
                    .downcast_ref::<RepoPathError>()
                    .is_some_and(|repo| !repo.is_outside())
            }),
            "the error must be a typed resolve failure, got {text}"
        );
    }

    #[test]
    fn raw_diff_pairs_a_rename_and_keeps_a_modify() {
        let data =
            ":100644 100644 aaa bbb R100\0old.md\0new.md\0:100644 100644 ccc ddd M\0note[1].md\0";
        let changes = parse_raw_z(data);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].old, "old.md");
        assert_eq!(changes[0].new, "new.md");
        assert_eq!(changes[1].old, "note[1].md");
        let specs = pathspecs_for(&changes, "new.md");
        assert_eq!(
            specs,
            vec![
                ":(literal)old.md".to_string(),
                ":(literal)new.md".to_string()
            ]
        );
        assert_eq!(
            pathspecs_for(&changes, "note[1].md"),
            vec![":(literal)note[1].md".to_string()]
        );
    }
}
