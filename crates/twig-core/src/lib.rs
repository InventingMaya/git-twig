//! Native CoW materialization with Git-owned linked-worktree bookkeeping.
mod native;
mod tree;

use anyhow::{Context, Result, bail, ensure};
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
use tree::{Entry, Kind};
pub type Error = anyhow::Error;

static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

/// Execute supported creation requests natively; delegate everything else to Git.
pub fn run(cwd: &Path, args: &[OsString]) -> Result<i32> {
    if let Some(request) = AddRequest::parse(args) {
        match prepare(cwd, &request) {
            Ok(Some(prepared)) => return install(cwd, args, prepared),
            Ok(None) => trace("stock Git: request is outside the CoW fast path"),
            Err(error) => trace(&format!("stock Git: {error:#}")),
        }
    }
    Ok(git(cwd)
        .arg("worktree")
        .args(args)
        .status()?
        .code()
        .unwrap_or(1))
}

fn trace(message: &str) {
    if env::var_os("TWIG_TRACE").is_some() {
        eprintln!("git-twig: {message}");
    }
}

#[derive(Debug)]
struct AddRequest {
    destination: PathBuf,
    revision: OsString,
}

impl AddRequest {
    fn parse(args: &[OsString]) -> Option<Self> {
        if args.first()? != "add" {
            return None;
        }
        let mut index = 1;
        if args.get(index)? == "-b" {
            let branch = args.get(index + 1)?;
            if branch.is_empty() || branch.to_string_lossy().starts_with('-') {
                return None;
            }
            index += 2;
        } else if args.get(index)? == "--detach" {
            index += 1;
        } else {
            // Omitted branch has Git's branch-name inference semantics. Delegate for now.
            return None;
        }
        let destination = args.get(index)?;
        if destination.is_empty() || destination.to_string_lossy().starts_with('-') {
            return None;
        }
        index += 1;
        let revision = args.get(index).cloned().unwrap_or_else(|| "HEAD".into());
        if revision.to_str().is_none()
            || revision.to_string_lossy().starts_with('-')
            || args.len() > index + 1
        {
            return None;
        }
        Some(Self {
            destination: destination.into(),
            revision,
        })
    }
}

struct Prepared {
    root: PathBuf,
    destination: PathBuf,
    stage: PathBuf,
    commit: String,
}

fn prepare(cwd: &Path, request: &AddRequest) -> Result<Option<Prepared>> {
    if !cfg!(target_os = "macos") {
        return Ok(None);
    }
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR",
    ] {
        if env::var_os(key).is_some() {
            return Ok(None);
        }
    }
    let root = output_path(checked(git(cwd).args(["rev-parse", "--show-toplevel"]))?.stdout)?;
    let commit = text(
        checked(
            git(&root)
                .arg("rev-parse")
                .arg("--verify")
                .arg(format!("{}^{{commit}}", request.revision.to_string_lossy())),
        )?
        .stdout,
    )?;
    let head = text(checked(git(&root).args(["rev-parse", "HEAD"]))?.stdout)?;
    if commit != head {
        return Ok(None);
    }
    if config_true(&root, "core.sparseCheckout") || config_true(&root, "core.sparseCheckoutCone") {
        return Ok(None);
    }
    let destination = if request.destination.is_absolute() {
        request.destination.clone()
    } else {
        cwd.join(&request.destination)
    };
    if destination.try_exists()? || fs::symlink_metadata(&destination).is_ok() {
        return Ok(None);
    }
    let parent = destination.parent().context("destination has no parent")?;
    if !parent.is_dir() {
        return Ok(None);
    }
    let parent = fs::canonicalize(parent)?;
    let destination = parent.join(destination.file_name().context("destination has no name")?);
    let autocrlf = git(&root)
        .args(["config", "--get", "core.autocrlf"])
        .output()?;
    if autocrlf.status.success() && autocrlf.stdout != b"false\n" {
        return Ok(None);
    }
    let entries =
        tree::parse(&checked(git(&root).args(["ls-tree", "-rz", "--full-tree", &commit]))?.stdout)?;
    if entries.iter().any(|entry| entry.kind == Kind::Submodule) {
        return Ok(None);
    }
    if has_conversion_attributes(&root, &entries)? {
        return Ok(None);
    }
    let stage = create_stage(&parent)?;
    trace(&format!("staging retained at {}", stage.display()));
    let data = stage.join("data");
    fs::create_dir(&data)?;
    for entry in &entries {
        materialize(&root, &data, entry)?;
    }
    verify(&root, &data, &stage.join("index"), &commit)?;
    Ok(Some(Prepared {
        root,
        destination,
        stage,
        commit,
    }))
}

fn materialize(root: &Path, destination: &Path, entry: &Entry) -> Result<()> {
    let source = root.join(&entry.path);
    // Reject replaced directory ancestors instead of following links out of the checkout.
    let mut ancestor = root.to_path_buf();
    for component in entry.path.parent().context("missing parent")?.components() {
        ancestor.push(component);
        ensure!(
            fs::symlink_metadata(&ancestor)?.is_dir(),
            "source ancestor is not a directory"
        );
    }
    let target = destination.join(&entry.path);
    fs::create_dir_all(target.parent().context("missing target parent")?)?;
    match entry.kind {
        Kind::File { executable } => {
            ensure!(
                fs::symlink_metadata(&source)?.is_file(),
                "source is not a regular file"
            );
            native::clone_file(&source, &target)?;
            ensure!(
                fs::symlink_metadata(&target)?.is_file(),
                "source type changed during clone"
            );
            native::set_executable(&target, executable)?;
        }
        Kind::Symlink => native::symlink(&fs::read_link(&source)?, &target)?,
        Kind::Submodule => bail!("submodules are not supported by the fast path"),
    }
    Ok(())
}

fn verify(root: &Path, data: &Path, index: &Path, commit: &str) -> Result<()> {
    let git_dir =
        output_path(checked(git(root).args(["rev-parse", "--absolute-git-dir"]))?.stdout)?;
    let staged_git = || {
        let mut command = git(root);
        command
            .env("GIT_DIR", &git_dir)
            .env("GIT_WORK_TREE", data)
            .env("GIT_INDEX_FILE", index)
            .args([
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.sparseCheckout=false",
                "-c",
                "core.filemode=true",
                "-c",
                "core.symlinks=true",
            ]);
        command
    };
    checked(staged_git().args(["read-tree", commit]))?;
    // Refresh hashes files with empty stat caches; raw diff-files alone can report stat-only changes.
    checked(staged_git().args(["update-index", "--really-refresh"]))
        .context("cloned files do not match the requested commit")?;
    checked(staged_git().args([
        "diff-files",
        "--quiet",
        "--no-ext-diff",
        "--ignore-submodules=none",
    ]))
    .context("cloned files do not match the requested commit")?;
    Ok(())
}

fn install(cwd: &Path, args: &[OsString], prepared: Prepared) -> Result<i32> {
    trace("APFS CoW: verified tracked files");
    // Preserve Git's branch validation and registration. No fallback after this mutation.
    let status = git(cwd)
        .args(["worktree", "add", "--no-checkout"])
        .args(&args[1..])
        .status()?;
    if !status.success() {
        return Ok(status.code().unwrap_or(1));
    }
    finish_install(&prepared).with_context(|| {
        format!(
            "partial worktree retained at {}; staging retained at {}; inspect before retrying",
            prepared.destination.display(),
            prepared.stage.display()
        )
    })?;
    let zero = "0".repeat(prepared.commit.len());
    let code = git(&prepared.destination)
        .args([
            "hook",
            "run",
            "--ignore-missing",
            "post-checkout",
            "--",
            &zero,
            &prepared.commit,
            "1",
        ])
        .status()?
        .code()
        .unwrap_or(1);
    Ok(code)
}

fn finish_install(prepared: &Prepared) -> Result<()> {
    // The ref may have moved while staging; never install files for a different HEAD.
    let actual = text(checked(git(&prepared.destination).args(["rev-parse", "HEAD"]))?.stdout)?;
    ensure!(
        actual == prepared.commit,
        "destination HEAD moved during creation"
    );
    for entry in fs::read_dir(prepared.stage.join("data"))? {
        let entry = entry?;
        let target = prepared.destination.join(entry.file_name());
        ensure!(
            fs::symlink_metadata(&target).is_err(),
            "destination changed during installation"
        );
        fs::rename(entry.path(), target)?;
    }
    checked(git(&prepared.destination).args(["read-tree", &prepared.commit]))?;
    trace(&format!("shared repository: {}", prepared.root.display()));
    Ok(())
}

fn has_conversion_attributes(root: &Path, entries: &[Entry]) -> Result<bool> {
    use std::io::Write;
    use std::process::Stdio;
    let mut command = git(root);
    command
        .args([
            "check-attr",
            "-z",
            "--stdin",
            "filter",
            "working-tree-encoding",
            "ident",
            "text",
            "eol",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let input: Vec<u8> = entries
        .iter()
        .flat_map(|entry| {
            let mut bytes = native::path_bytes(&entry.path);
            bytes.push(0);
            bytes
        })
        .collect();
    // Drain stdout concurrently: attribute output can exceed pipe capacity before stdin is consumed.
    let mut stdin = child.stdin.take().context("missing Git stdin")?;
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let result = child.wait_with_output()?;
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("attribute writer panicked"))??;
    ensure!(result.status.success(), "git check-attr failed");
    let fields: Vec<_> = result
        .stdout
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .collect();
    ensure!(fields.len() % 3 == 0, "invalid Git attribute response");
    Ok(fields
        .chunks_exact(3)
        .any(|row| row[2] != b"unspecified" && row[2] != b"unset"))
}

fn config_true(root: &Path, key: &str) -> bool {
    git(root)
        .args(["config", "--bool", "--get", key])
        .output()
        .is_ok_and(|result| result.stdout == b"true\n")
}

fn create_stage(parent: &Path) -> Result<PathBuf> {
    for _ in 0..1000 {
        let path = parent.join(format!(
            ".git-twig-stage-{}-{}",
            std::process::id(),
            NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("unable to allocate staging directory")
}

fn git(cwd: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(cwd);
    command
}

fn checked(command: &mut Command) -> Result<Output> {
    let output = command.output().context("cannot execute Git")?;
    ensure!(
        output.status.success(),
        "Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}

fn text(mut bytes: Vec<u8>) -> Result<String> {
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    Ok(String::from_utf8(bytes)?)
}

fn output_path(mut bytes: Vec<u8>) -> Result<PathBuf> {
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    Ok(PathBuf::from(native::os_string(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(words: &[&str]) -> bool {
        AddRequest::parse(&words.iter().map(OsString::from).collect::<Vec<_>>()).is_some()
    }
    #[test]
    fn parses_only_the_supported_subset() {
        assert!(parse(&["add", "-b", "feature", "../feature"]));
        assert!(parse(&["add", "--detach", "../feature", "HEAD"]));
        for words in [
            vec!["add"],
            vec!["add", "-b"],
            vec!["add", "-B", "a", "b"],
            vec!["add", "-b", "a", "b", "HEAD", "extra"],
            vec!["list"],
            vec!["add", "-b", "a", "--help"],
        ] {
            assert!(!parse(&words));
        }
    }
}
