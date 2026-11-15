use anyhow::{Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Result<Self> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-fixtures")
            .join(format!(
                "{}-{}-{}",
                std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&root)?;
        let root = fs::canonicalize(root)?;
        let source = root.join("source");
        fs::create_dir(&source)?;
        git(&source, &["init", "-b", "main"])?;
        git(&source, &["config", "user.name", "Fixture"])?;
        git(
            &source,
            &["config", "user.email", "fixture@example.invalid"],
        )?;
        fs::write(source.join("hello.txt"), "hello\n")?;
        fs::write(source.join(".gitignore"), "ignored/\n")?;
        git(&source, &["add", "."])?;
        git(&source, &["commit", "-m", "seed"])?;
        eprintln!("fixture retained: {}", root.display());
        Ok(Self { root, source })
    }
    fn twig(&self, args: &[&str]) -> Result<Output> {
        twig(&self.source, args)
    }
    fn add(&self) -> Result<Output> {
        self.twig(&["add", "-b", "feature", "../feature", "HEAD"])
    }
    fn destination(&self) -> PathBuf {
        self.root.join("feature")
    }
}
fn command(program: impl AsRef<std::ffi::OsStr>, cwd: &Path) -> Command {
    let mut c = Command::new(program);
    c.current_dir(cwd)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("TWIG_TRACE", "1");
    c
}
fn checked(output: Output) -> Result<Output> {
    ensure!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}
fn git(cwd: &Path, args: &[&str]) -> Result<Output> {
    checked(command("git", cwd).args(args).output()?)
}
fn twig(cwd: &Path, args: &[&str]) -> Result<Output> {
    Ok(command(env!("CARGO_BIN_EXE_git-twig"), cwd)
        .args(args)
        .output()?)
}
fn clean(path: &Path) -> Result<()> {
    ensure!(
        git(path, &["status", "--porcelain"])?.stdout.is_empty(),
        "worktree is dirty"
    );
    Ok(())
}
fn native(output: &Output) {
    if cfg!(target_os = "macos") {
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("APFS CoW: verified tracked files"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn creates_real_clean_worktree_without_source_hardlinks() -> Result<()> {
    let f = Fixture::new()?;
    let output = checked(f.add()?)?;
    native(&output);
    clean(&f.destination())?;
    assert_eq!(
        git(&f.destination(), &["branch", "--show-current"])?.stdout,
        b"feature\n"
    );
    assert!(
        git(&f.source, &["worktree", "list", "--porcelain"])?
            .stdout
            .windows(7)
            .any(|w| w == b"feature")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_ne!(
            fs::metadata(f.source.join("hello.txt"))?.ino(),
            fs::metadata(f.destination().join("hello.txt"))?.ino()
        );
    }
    fs::write(f.destination().join("hello.txt"), "private\n")?;
    assert_eq!(fs::read(f.source.join("hello.txt"))?, b"hello\n");
    Ok(())
}

#[test]
fn dirty_source_produces_clean_commit_checkout() -> Result<()> {
    let f = Fixture::new()?;
    fs::write(f.source.join("hello.txt"), "dirty\n")?;
    checked(f.add()?)?;
    clean(&f.destination())?;
    assert_eq!(fs::read(f.destination().join("hello.txt"))?, b"hello\n");
    assert_eq!(fs::read(f.source.join("hello.txt"))?, b"dirty\n");
    Ok(())
}

#[test]
fn staged_source_changes_are_not_inherited() -> Result<()> {
    let f = Fixture::new()?;
    fs::write(f.source.join("hello.txt"), "staged\n")?;
    git(&f.source, &["add", "."])?;
    checked(f.add()?)?;
    clean(&f.destination())?;
    assert_eq!(fs::read(f.destination().join("hello.txt"))?, b"hello\n");
    assert!(
        !git(&f.source, &["diff", "--cached", "--name-only"])?
            .stdout
            .is_empty()
    );
    Ok(())
}

#[test]
fn untracked_and_ignored_files_stay_out() -> Result<()> {
    let f = Fixture::new()?;
    fs::create_dir(f.source.join("ignored"))?;
    fs::write(f.source.join("ignored/cache"), "cache")?;
    fs::write(f.source.join("untracked"), "secret")?;
    let output = checked(f.add()?)?;
    native(&output);
    assert!(!f.destination().join("ignored").exists());
    assert!(!f.destination().join("untracked").exists());
    clean(&f.destination())
}

#[test]
fn existing_destination_is_untouched() -> Result<()> {
    let f = Fixture::new()?;
    fs::create_dir(f.destination())?;
    fs::write(f.destination().join("keep"), "keep")?;
    assert!(!f.add()?.status.success());
    assert_eq!(fs::read(f.destination().join("keep"))?, b"keep");
    Ok(())
}

#[test]
fn different_commit_uses_git() -> Result<()> {
    let f = Fixture::new()?;
    fs::write(f.source.join("hello.txt"), "second\n")?;
    git(&f.source, &["commit", "-am", "second"])?;
    checked(f.twig(&["add", "--detach", "../feature", "HEAD~1"])?)?;
    assert_eq!(fs::read(f.destination().join("hello.txt"))?, b"hello\n");
    clean(&f.destination())
}

#[test]
fn can_clone_a_linked_source() -> Result<()> {
    let f = Fixture::new()?;
    checked(f.add()?)?;
    let output = checked(twig(
        &f.destination(),
        &["add", "-b", "second", "../second"],
    )?)?;
    native(&output);
    clean(&f.root.join("second"))
}

#[test]
fn detached_and_no_checkout_modes() -> Result<()> {
    let f = Fixture::new()?;
    let output = checked(f.twig(&["add", "--detach", "../feature"])?)?;
    native(&output);
    clean(&f.destination())?;
    assert!(
        git(&f.destination(), &["branch", "--show-current"])?
            .stdout
            .is_empty()
    );
    checked(f.twig(&["add", "--no-checkout", "--detach", "../empty"])?)?;
    assert!(!f.root.join("empty/hello.txt").exists());
    Ok(())
}

#[test]
fn passthrough_matches_git() -> Result<()> {
    let f = Fixture::new()?;
    checked(f.add()?)?;
    assert_eq!(
        checked(f.twig(&["list", "--porcelain"])?)?.stdout,
        git(&f.source, &["worktree", "list", "--porcelain"])?.stdout
    );
    let twig = f.twig(&["not-a-command"])?;
    let stock = command("git", &f.source)
        .args(["worktree", "not-a-command"])
        .output()?;
    assert_eq!(twig.status.code(), stock.status.code());
    Ok(())
}

#[cfg(unix)]
#[test]
fn executable_symlink_and_unusual_names() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new()?;
    let odd = std::ffi::OsString::from("odd-雪\t\n");
    fs::write(f.source.join(&odd), "odd")?;
    fs::write(f.source.join("run"), "#!/bin/sh\nexit 0\n")?;
    fs::set_permissions(f.source.join("run"), fs::Permissions::from_mode(0o755))?;
    std::os::unix::fs::symlink("missing", f.source.join("link"))?;
    git(&f.source, &["add", "."])?;
    git(&f.source, &["commit", "-m", "modes"])?;
    let output = checked(f.add()?)?;
    native(&output);
    assert_eq!(fs::read(f.destination().join(odd))?, b"odd");
    assert_eq!(
        fs::read_link(f.destination().join("link"))?,
        PathBuf::from("missing")
    );
    assert_eq!(
        fs::metadata(f.destination().join("run"))?
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    clean(&f.destination())
}

#[cfg(unix)]
#[test]
fn hooks_run_once_and_failure_retains_worktree() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for failure in [false, true] {
        let f = Fixture::new()?;
        let hook = f.source.join(".git/hooks/post-checkout");
        fs::write(
            &hook,
            format!(
                "#!/bin/sh\nprintf '%s %s %s\\n' \"$1\" \"$2\" \"$3\" >> hook-output\nexit {}\n",
                if failure { 23 } else { 0 }
            ),
        )?;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
        let output = f.add()?;
        assert_eq!(output.status.code(), Some(if failure { 23 } else { 0 }));
        let contents = fs::read_to_string(f.destination().join("hook-output"))?;
        assert_eq!(contents.lines().count(), 1);
        assert!(contents.starts_with(&"0".repeat(40)));
        assert!(contents.ends_with(" 1\n"));
        assert!(f.destination().join(".git").is_file());
        assert!(!f.source.join("hook-output").exists());
    }
    Ok(())
}

#[test]
fn filters_and_sparse_checkouts_delegate() -> Result<()> {
    for sparse in [false, true] {
        let f = Fixture::new()?;
        if sparse {
            git(&f.source, &["sparse-checkout", "init", "--cone"])?;
        } else {
            fs::write(f.source.join(".gitattributes"), "*.txt ident\n")?;
            git(&f.source, &["add", "."])?;
            git(&f.source, &["commit", "-m", "attributes"])?;
        }
        let output = checked(f.add()?)?;
        assert!(!String::from_utf8_lossy(&output.stderr).contains("APFS CoW:"));
        clean(&f.destination())?;
    }
    Ok(())
}

#[test]
fn concurrent_creators_do_not_overwrite_each_other() -> Result<()> {
    let f = Fixture::new()?;
    let a = f.source.clone();
    let b = a.clone();
    let first = std::thread::spawn(move || twig(&a, &["add", "--detach", "../feature"]));
    let second = std::thread::spawn(move || twig(&b, &["add", "--detach", "../feature"]));
    let a = first.join().unwrap()?;
    let b = second.join().unwrap()?;
    assert_ne!(a.status.success(), b.status.success());
    clean(&f.destination())
}
