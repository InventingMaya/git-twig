//! Real-repository cohorts: isolated images for allocation, host APFS for representative timing.
//! Only image-owned paths are removed. Original repositories are read-only inputs.
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{
    env,
    ffi::{CString, OsString},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Filesystem {
    IsolatedImage,
    CaseSensitiveImage,
    Host,
}

impl Filesystem {
    fn snapshot(self, path: &Path) -> Result<Option<Space>> {
        match self {
            Self::IsolatedImage | Self::CaseSensitiveImage => Ok(Some(settled_space(path)?)),
            Self::Host => Ok(None),
        }
    }
}

#[derive(Clone, Serialize)]
struct Repository {
    label: String,
    #[serde(skip)]
    path: PathBuf,
    commit: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Suite {
    schema_version: u32,
    filesystem: Filesystem,
    image_size_gib: Option<u64>,
    warmup: bool,
    selected_backend: Option<String>,
    suite: String,
    generated_at_unix_ms: u128,
    commit: String,
    dirty: bool,
    candidate_sha256: String,
    git_version: String,
    platform: String,
    cpu: String,
    concurrency: usize,
    worktrees_per_cohort: usize,
    repeats: usize,
    repositories: Vec<Repository>,
    notes: Vec<String>,
    cohorts: Vec<Cohort>,
    failures: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Cohort {
    repository: String,
    source_commit: String,
    source_tree: String,
    backend: String,
    repeat: usize,
    tracked_files: usize,
    logical_bytes_per_worktree: u64,
    create_wall_ms: f64,
    creates_per_second: f64,
    status_and_verification_wall_ms: f64,
    succeeded: usize,
    failed: usize,
    verified_files: usize,
    source_unchanged: bool,
    private_write_tested: bool,
    before: Option<Space>,
    after_create: Option<Space>,
    after_status: Option<Space>,
    after_private_writes: Option<Space>,
    growth_after_create_bytes: Option<i128>,
    growth_after_status_bytes: Option<i128>,
    private_write_growth_bytes: Option<i128>,
    operations: Vec<Operation>,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Space {
    allocated_bytes: u64,
    total_bytes: u64,
    stable: bool,
    observed_allocated_bytes: Vec<u64>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Operation {
    worker: usize,
    create_ms: f64,
    create_started_at_ms: f64,
    create_finished_at_ms: f64,
    first_status_ms: Option<f64>,
    exit_code: Option<i32>,
    native_cow: bool,
    verified: bool,
    error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreationProgress<'a> {
    schema_version: u32,
    phase: &'static str,
    repository: &'a str,
    source_commit: &'a str,
    backend: &'a str,
    repeat: usize,
    expected_operations: usize,
    elapsed_ms: f64,
    operations: &'a [Operation],
}

struct Expected {
    path: PathBuf,
    data: Vec<u8>,
    symlink: bool,
    executable: bool,
}
#[derive(Clone, Copy)]
struct CohortOptions {
    filesystem: Filesystem,
    warmup: bool,
    worktrees: usize,
}

struct Image {
    path: PathBuf,
    mount: PathBuf,
    attached: bool,
    filesystem: Filesystem,
    size_gib: u64,
}

fn main() -> Result<()> {
    if env::args().any(|arg| arg == "--help") {
        println!(
            "twig-real-bench --repo label=/absolute/repo [--repo ...] --concurrency 100 --repeats 3 --output target/real-results/run.json\nSequential example: --worktrees 10 --concurrency 1 --repeats 1\nOptional: --filesystem host (retains fixtures, does not measure allocation), --revision label=COMMIT (after --repo).\nmacOS only. --filesystem case-sensitive-image supports case-colliding trees; --image-gib configures sparse image capacity (default 8). Images are detached and deleted after measurement.\nOriginal repositories are read-only inputs. Results default to target/ (not published)."
        );
        return Ok(());
    }
    ensure!(
        cfg!(target_os = "macos"),
        "isolated APFS benchmarks require macOS"
    );
    let cwd = env::current_dir()?;
    let mut repositories = Vec::new();
    let mut concurrency = 100;
    let mut worktrees = None;
    let mut repeats = 3;
    let mut output = None;
    let mut candidate_path = cwd.join("target/release/git-twig");
    let mut filesystem = Filesystem::IsolatedImage;
    let mut image_size_gib: u64 = 8;
    let mut warmup = true;
    let mut selected_backend = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args.next().context("option requires a value")?;
        match arg.as_str() {
            "--repo" => {
                let (label, path) = value.split_once('=').context("repo must be label=path")?;
                ensure!(
                    !label.is_empty()
                        && label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
                    "label must use letters, digits or hyphens"
                );
                ensure!(
                    !repositories.iter().any(|r: &Repository| r.label == label),
                    "duplicate repository label"
                );
                let path = fs::canonicalize(path)?;
                let commit = git_text(&path, &["rev-parse", "HEAD"])?;
                repositories.push(Repository {
                    label: label.into(),
                    path,
                    commit,
                });
            }
            "--revision" => {
                let (label, commit) = value
                    .split_once('=')
                    .context("revision must be label=COMMIT")?;
                ensure!(
                    (commit.len() == 40 || commit.len() == 64)
                        && commit.bytes().all(|b| b.is_ascii_hexdigit()),
                    "revision must be a full commit OID"
                );
                let repository = repositories
                    .iter_mut()
                    .find(|r| r.label == label)
                    .context("declare --repo before --revision")?;
                repository.commit = git_text(
                    &repository.path,
                    &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
                )?;
            }
            "--filesystem" => {
                filesystem = match value.as_str() {
                    "host" => Filesystem::Host,
                    "isolated-image" => Filesystem::IsolatedImage,
                    "case-sensitive-image" => Filesystem::CaseSensitiveImage,
                    _ => anyhow::bail!(
                        "filesystem must be host, isolated-image or case-sensitive-image"
                    ),
                }
            }
            "--warmup" => warmup = value.parse()?,
            "--backend" => {
                ensure!(
                    value == "git" || value == "twig",
                    "backend must be git or twig"
                );
                selected_backend = Some(value);
            }
            "--image-gib" => image_size_gib = value.parse()?,
            "--concurrency" => concurrency = value.parse()?,
            "--worktrees" => worktrees = Some(value.parse::<usize>()?),
            "--repeats" => repeats = value.parse()?,
            "--output" => output = Some(PathBuf::from(value)),
            "--candidate" => candidate_path = PathBuf::from(value),
            _ => anyhow::bail!("unknown option: {arg}"),
        }
    }
    let worktrees = worktrees.unwrap_or(concurrency);
    ensure!(
        (1..=1000).contains(&worktrees) && concurrency <= worktrees,
        "worktrees must be 1..1000 and at least the concurrency"
    );
    ensure!(
        !repositories.is_empty()
            && (1..=128).contains(&concurrency)
            && repeats > 0
            && (4..=256).contains(&image_size_gib),
        "provide repositories, concurrency 1..128, positive repeats, image capacity 4..256 GiB"
    );
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let output =
        output.unwrap_or_else(|| cwd.join(format!("target/real-results/{timestamp}.json")));
    ensure!(!output.exists(), "refusing to overwrite an existing run");
    let root = cwd.join(format!(
        "target/real-images/{timestamp}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root)?;
    let root = fs::canonicalize(root)?;
    let candidate = fs::canonicalize(candidate_path)?;
    let digest = capture(
        &cwd,
        "shasum",
        &[
            OsString::from("-a"),
            "256".into(),
            candidate.as_os_str().into(),
        ],
    )?;
    let mut suite = Suite {
        schema_version: 2, filesystem, warmup, selected_backend: selected_backend.clone(), image_size_gib: if filesystem == Filesystem::Host { None } else { Some(image_size_gib) }, suite: "real-apfs-cohort".into(), generated_at_unix_ms: timestamp,
        commit: git_text(&cwd, &["rev-parse", "HEAD"])?, dirty: !git_text(&cwd, &["status", "--porcelain"])?.is_empty(),
        candidate_sha256: digest.split_whitespace().next().context("missing candidate hash")?.into(),
        git_version: capture(&cwd, "git", &["--version".into()])?,
        platform: format!("{} {} {}", env::consts::OS, env::consts::ARCH, capture(&cwd, "uname", &["-r".into()])?),
        cpu: capture(&cwd, "sysctl", &["-n".into(), "machdep.cpu.brand_string".into()])?, concurrency, worktrees_per_cohort: worktrees, repeats, repositories: repositories.clone(),
        notes: vec![
            "Each image-backed backend/repeat gets a fresh, isolated sparse APFS image (capacity/format recorded in imageSizeGiB/filesystem). Seed uses git clone --shared --no-checkout plus explicit pinned-commit checkout. Original repositories and object stores are read-only inputs; seed history uses alternates outside image.".into(),
            if warmup { "One operation warmup per backend; removed before image space snapshots, retained on host. Global/system Git config and hooks disabled for fixture setup; no ignored/untracked dependencies or build output copied. Backend order alternates by repeat.".into() } else { "Operation warmup disabled; seed checkout still warms input objects/files. Global/system Git config and fixture hooks disabled.".into() },
            "A barrier releases the bounded worker pool simultaneously; each worker starts its next queued create only after its previous CLI returns. worktreesPerCohort is the total count; concurrency is the maximum in flight. Create wall time includes thread startup/barrier/join, per-operation latency starts at barrier release. No verification/generation competes with creates. No retries: failures are data.".into(),
            "First git status is measured sequentially after all creates and the initial space snapshot. Full byte/symlink/executable verification follows each first status; its CPU is outside creation timing. A first-status sum is not a concurrent-status wall time.".into(),
            "Allocated space is total minus free blocks from statvfs on the isolated APFS filesystem, sampled after sync until stable (or marked unstable). This includes working files, indexes, retained staging and filesystem metadata, not logical size/du/st_blocks. Host sparse-image allocation is not the metric.".into(),
            "After initial verification/space measurement, one byte in one regular file per successful checkout is changed in place. Additional filesystem allocation is sampled; seed file must remain unchanged. Images are detached and deleted after saving each cohort; no benchmark input paths or source file contents are serialized.".into(),
            "Local warm-cache results. A sparse APFS image is a controlled filesystem, not a claim about native host-volume maximum throughput. Report failures alongside latency and allocation; do not compare unequal successful counts as identical capacity.".into(),
        ], cohorts: vec![], failures: vec![],
    };
    if filesystem == Filesystem::Host {
        suite.notes.push("Filesystem=host overrides image setup/cleanup notes: fresh cohort directories and warmup worktrees are retained, no allocation snapshots/deltas are reported, no sync is forced. Use isolated-image runs for space comparisons. Input revisions can be pinned to match earlier image runs.".into());
    }
    save(&output, &suite)?;
    for repository in &repositories {
        for repeat in 0..repeats {
            let order = if repeat % 2 == 0 {
                ["git", "twig"]
            } else {
                ["twig", "git"]
            };
            for backend in order {
                if selected_backend
                    .as_deref()
                    .is_some_and(|chosen| chosen != backend)
                {
                    continue;
                }
                eprintln!(
                    "{}: {backend}, repeat {}/{repeats}, {worktrees} creates with concurrency {concurrency}",
                    repository.label,
                    repeat + 1
                );
                let stem = root.join(format!("{}-{backend}-{repeat}", repository.label));
                let mut image = Image::new(&stem, filesystem, image_size_gib);
                let prepare = match filesystem {
                    Filesystem::IsolatedImage | Filesystem::CaseSensitiveImage => image.attach(),
                    Filesystem::Host => fs::create_dir(&image.mount).map_err(Into::into),
                };
                let result = prepare.and_then(|()| {
                    cohort(
                        repository,
                        backend,
                        repeat,
                        concurrency,
                        &candidate,
                        &image.mount,
                        CohortOptions {
                            filesystem,
                            warmup,
                            worktrees,
                        },
                    )
                });
                match result {
                    Ok(point) => {
                        eprintln!(
                            "  {:.2}s creates; {} ok / {} failed; allocation after status: {}",
                            point.create_wall_ms / 1000.0,
                            point.succeeded,
                            point.failed,
                            point
                                .growth_after_status_bytes
                                .map(|bytes| format!("{:.2} MiB", bytes as f64 / 1048576.0))
                                .unwrap_or_else(|| "unmeasured on shared host filesystem".into())
                        );
                        suite.cohorts.push(point);
                    }
                    Err(error) => suite.failures.push(format!(
                        "{} / {backend} / repeat {repeat}: {error:#}",
                        repository.label
                    )),
                }
                save(&output, &suite)?;
                if filesystem != Filesystem::Host {
                    image.cleanup()?;
                } else {
                    eprintln!("  host fixtures retained at {}", image.mount.display());
                }
                ensure!(
                    suite.failures.is_empty(),
                    "setup/measurement failure saved to {}",
                    output.display()
                );
            }
        }
    }
    println!("{}", output.display());
    Ok(())
}

impl Image {
    fn new(stem: &Path, filesystem: Filesystem, size_gib: u64) -> Self {
        Self {
            path: stem.with_extension("sparseimage"),
            mount: stem.with_extension("mount"),
            attached: false,
            filesystem,
            size_gib,
        }
    }
    fn attach(&mut self) -> Result<()> {
        checked(
            Command::new("hdiutil")
                .args(["create", "-quiet", "-size"])
                .arg(format!("{}g", self.size_gib))
                .args(["-type", "SPARSE", "-fs"])
                .arg(if self.filesystem == Filesystem::CaseSensitiveImage {
                    "Case-sensitive APFS"
                } else {
                    "APFS"
                })
                .args(["-volname", "TwigBench", "-nospotlight"])
                .arg(&self.path)
                .output()?,
        )?;
        fs::create_dir(&self.mount)?;
        checked(
            Command::new("hdiutil")
                .args([
                    "attach",
                    "-quiet",
                    "-nobrowse",
                    "-owners",
                    "on",
                    "-mountpoint",
                ])
                .arg(&self.mount)
                .arg(&self.path)
                .output()?,
        )?;
        self.attached = true;
        fs::write(self.mount.join(".metadata_never_index"), [])?;
        Ok(())
    }
    fn cleanup(&mut self) -> Result<()> {
        if self.attached {
            checked(
                Command::new("hdiutil")
                    .arg("detach")
                    .arg(&self.mount)
                    .output()?,
            )
            .with_context(|| {
                format!(
                    "image still mounted at {}; retained {}",
                    self.mount.display(),
                    self.path.display()
                )
            })?;
            self.attached = false;
        }
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        eprintln!("  disposable image detached and removed");
        Ok(())
    }
}

fn cohort(
    repository: &Repository,
    backend: &str,
    repeat: usize,
    concurrency: usize,
    candidate: &Path,
    mount: &Path,
    options: CohortOptions,
) -> Result<Cohort> {
    let filesystem = options.filesystem;
    let seed = mount.join("seed");
    checked(
        command("git", mount)
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "clone",
                "--shared",
                "--no-checkout",
                "--",
            ])
            .arg(&repository.path)
            .arg(&seed)
            .output()?,
    )?;
    checked(
        command("git", &seed)
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "checkout",
                "--detach",
                &repository.commit,
            ])
            .output()?,
    )?;
    let tree = git_text(&seed, &["rev-parse", "HEAD^{tree}"])?;
    let expected = expected_files(&seed)?;
    let logical_bytes = expected.iter().map(|entry| entry.data.len() as u64).sum();
    if options.warmup {
        let warmup = create(
            &seed,
            &mount.join("warmup"),
            candidate,
            backend,
            0,
            Instant::now(),
        );
        ensure!(
            warmup.error.is_none() && (backend != "twig" || warmup.native_cow),
            "warmup did not use expected backend: {:?}",
            warmup.error
        );
        if filesystem != Filesystem::Host {
            checked(
                command("git", &seed)
                    .args(["worktree", "remove", "--force"])
                    .arg(mount.join("warmup"))
                    .output()?,
            )?;
            // Only benchmark-owned warmup staging paths on this disposable image are removed.
            for entry in fs::read_dir(mount)? {
                let entry = entry?;
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".git-twig-stage-")
                {
                    fs::remove_dir_all(entry.path())?;
                }
            }
        }
    }
    let before = filesystem.snapshot(mount)?;
    let barrier = Arc::new(Barrier::new(concurrency));
    let next = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = mpsc::channel();
    let checkpoint = mount.with_extension("creation.json");
    let start = Instant::now();
    let handles = (0..concurrency)
        .map(|_| {
            let seed = seed.clone();
            let mount = mount.to_path_buf();
            let candidate = candidate.to_path_buf();
            let backend = backend.to_owned();
            let barrier = Arc::clone(&barrier);
            let next = Arc::clone(&next);
            let sender = sender.clone();
            thread::spawn(move || {
                barrier.wait();
                loop {
                    let worker = next.fetch_add(1, Ordering::Relaxed);
                    if worker >= options.worktrees {
                        break;
                    }
                    let destination = mount.join(format!("w-{worker}"));
                    let outcome = create(&seed, &destination, &candidate, &backend, worker, start);
                    if sender.send(outcome).is_err() {
                        break;
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    eprintln!(
        "cohort workers launched: {concurrency}; queued creates: {}",
        options.worktrees
    );
    drop(sender);
    let mut operations = Vec::with_capacity(options.worktrees);
    for mut operation in receiver {
        if let Some(error) = &mut operation.error {
            *error = error
                .replace(&mount.display().to_string(), "<image>")
                .replace(&repository.path.display().to_string(), "<source>");
        }
        operations.push(operation);
        eprintln!(
            "{backend}: CLI completed {}/{}; content verification pending",
            operations.len(),
            options.worktrees
        );
        let progress = CreationProgress {
            schema_version: 1,
            phase: "creation; verification pending",
            repository: &repository.label,
            source_commit: &repository.commit,
            backend,
            repeat,
            expected_operations: options.worktrees,
            elapsed_ms: ms(start),
            operations: &operations,
        };
        save_creation_progress(&checkpoint, &progress)?;
    }
    for handle in handles {
        handle.join().expect("CLI worker panicked");
    }
    ensure!(
        operations.len() == options.worktrees,
        "a CLI worker did not return an outcome"
    );
    operations.sort_by_key(|operation| operation.worker);
    if concurrency == 1 {
        ensure!(
            operations
                .windows(2)
                .all(|pair| pair[0].create_finished_at_ms <= pair[1].create_started_at_ms),
            "sequential creates overlapped"
        );
    }
    let create_wall_ms = ms(start);
    let after_create = filesystem.snapshot(mount)?;
    let status_start = Instant::now();
    let mut verified_files = 0;
    for operation in &mut operations {
        if operation.error.is_some() {
            continue;
        }
        let destination = mount.join(format!("w-{}", operation.worker));
        let verification = (|| -> Result<()> {
            let start = Instant::now();
            let status = checked(
                command("git", &destination)
                    .args(["status", "--porcelain", "--untracked-files=no"])
                    .output()?,
            )?;
            operation.first_status_ms = Some(ms(start));
            ensure!(status.stdout.is_empty(), "checkout is dirty");
            for entry in &expected {
                verify_file(&destination, entry)?;
                verified_files += 1;
            }
            ensure!(
                backend != "twig" || operation.native_cow,
                "Twig delegated to Git rather than using CoW"
            );
            operation.verified = true;
            Ok(())
        })();
        if let Err(error) = verification {
            operation.error = Some(format!("{error:#}"));
        }
    }
    let status_and_verification_wall_ms = ms(status_start);
    let after_status = filesystem.snapshot(mount)?;
    let first = expected.iter().find(|entry| !entry.symlink);
    if let Some(first) = first {
        for operation in &mut operations {
            if !operation.verified {
                continue;
            }
            let destination = mount
                .join(format!("w-{}", operation.worker))
                .join(&first.path);
            let write = (|| -> Result<()> {
                OpenOptions::new()
                    .write(true)
                    .open(destination)?
                    .write_all(&[first.data.first().map_or(0x42, |byte| byte ^ 0xff)])?;
                Ok(())
            })();
            if let Err(error) = write {
                operation.error = Some(format!("private write: {error:#}"));
                operation.verified = false;
            }
        }
    }
    let after_private_writes = if first.is_some() {
        filesystem.snapshot(mount)?
    } else {
        None
    };
    let source_unchanged = match first {
        Some(first) => fs::read(seed.join(&first.path))? == first.data,
        None => git_text(&seed, &["status", "--porcelain"])?.is_empty(),
    };
    ensure!(source_unchanged, "seed changed after private writes");
    for operation in &mut operations {
        if let Some(error) = &mut operation.error {
            *error = error
                .replace(&mount.display().to_string(), "<image>")
                .replace(&repository.path.display().to_string(), "<source>");
        }
    }
    let succeeded = operations
        .iter()
        .filter(|op| op.verified && op.error.is_none())
        .count();
    Ok(Cohort {
        repository: repository.label.clone(),
        source_commit: repository.commit.clone(),
        source_tree: tree,
        backend: backend.into(),
        repeat,
        tracked_files: expected.len(),
        logical_bytes_per_worktree: logical_bytes,
        create_wall_ms,
        creates_per_second: succeeded as f64 * 1000.0 / create_wall_ms,
        status_and_verification_wall_ms,
        succeeded,
        failed: options.worktrees - succeeded,
        verified_files,
        source_unchanged,
        private_write_tested: first.is_some(),
        growth_after_create_bytes: allocation_delta(&after_create, &before),
        growth_after_status_bytes: allocation_delta(&after_status, &before),
        private_write_growth_bytes: allocation_delta(&after_private_writes, &after_status),
        before,
        after_create,
        after_status,
        after_private_writes,
        operations,
    })
}

fn save_creation_progress(path: &Path, progress: &CreationProgress<'_>) -> Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(progress)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn allocation_delta(after: &Option<Space>, before: &Option<Space>) -> Option<i128> {
    after
        .as_ref()
        .zip(before.as_ref())
        .map(|(after, before)| after.allocated_bytes as i128 - before.allocated_bytes as i128)
}

fn create(
    seed: &Path,
    destination: &Path,
    candidate: &Path,
    backend: &str,
    worker: usize,
    cohort_start: Instant,
) -> Operation {
    let mut command = if backend == "git" {
        command("git", seed)
    } else {
        command(candidate, seed)
    };
    if backend == "git" {
        command.arg("worktree");
    }
    command
        .args(["add", "--detach"])
        .arg(destination)
        .arg("HEAD");
    let start = Instant::now();
    let create_started_at_ms = start.duration_since(cohort_start).as_secs_f64() * 1000.0;
    let output = command.output();
    let elapsed = ms(start);
    let create_finished_at_ms = ms(cohort_start);
    match output {
        Ok(output) => Operation {
            worker,
            create_ms: elapsed,
            create_started_at_ms,
            create_finished_at_ms,
            first_status_ms: None,
            exit_code: output.status.code(),
            native_cow: String::from_utf8_lossy(&output.stderr)
                .contains("APFS CoW: verified tracked files"),
            verified: false,
            error: if output.status.success() {
                None
            } else {
                Some(String::from_utf8_lossy(&output.stderr).into_owned())
            },
        },
        Err(error) => Operation {
            worker,
            create_ms: elapsed,
            create_started_at_ms,
            create_finished_at_ms,
            first_status_ms: None,
            exit_code: None,
            native_cow: false,
            verified: false,
            error: Some(error.to_string()),
        },
    }
}

fn expected_files(seed: &Path) -> Result<Vec<Expected>> {
    use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
    let paths = checked(command("git", seed).args(["ls-files", "-z"]).output()?)?.stdout;
    paths
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            let path = PathBuf::from(OsString::from_vec(path.to_vec()));
            let absolute = seed.join(&path);
            let metadata = fs::symlink_metadata(&absolute)?;
            let symlink = metadata.file_type().is_symlink();
            let data = if symlink {
                use std::os::unix::ffi::OsStrExt;
                fs::read_link(&absolute)?.as_os_str().as_bytes().to_vec()
            } else {
                ensure!(
                    metadata.is_file(),
                    "submodules/special files are not supported in this suite"
                );
                fs::read(&absolute)?
            };
            Ok(Expected {
                path,
                data,
                symlink,
                executable: metadata.permissions().mode() & 0o100 != 0,
            })
        })
        .collect()
}
fn verify_file(root: &Path, entry: &Expected) -> Result<()> {
    use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
    let absolute = root.join(&entry.path);
    let metadata = fs::symlink_metadata(&absolute)?;
    ensure!(
        metadata.file_type().is_symlink() == entry.symlink,
        "entry type differs"
    );
    if entry.symlink {
        ensure!(
            fs::read_link(absolute)?.as_os_str().as_bytes() == entry.data,
            "symlink differs"
        );
    } else {
        ensure!(fs::read(absolute)? == entry.data, "file bytes differ");
        ensure!(
            (metadata.permissions().mode() & 0o100 != 0) == entry.executable,
            "executable bit differs"
        );
    }
    Ok(())
}
fn settled_space(mount: &Path) -> Result<Space> {
    checked(Command::new("sync").output()?)?;
    let mut observations = vec![];
    let mut snapshot = space(mount)?;
    for _ in 0..12 {
        thread::sleep(Duration::from_millis(250));
        snapshot = space(mount)?;
        observations.push(snapshot.allocated_bytes);
        if observations.len() >= 3
            && observations[observations.len() - 3..]
                .iter()
                .all(|n| *n == snapshot.allocated_bytes)
        {
            snapshot.stable = true;
            break;
        }
    }
    snapshot.observed_allocated_bytes = observations;
    Ok(snapshot)
}
// libc block counters are u32 on Darwin and u64 on Linux.
#[allow(clippy::unnecessary_cast)]
fn space(mount: &Path) -> Result<Space> {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(mount.as_os_str().as_bytes())?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: path is a live C string; stat points to writable storage of the required size.
    ensure!(
        unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } == 0,
        "statvfs failed: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: successful statvfs initialized the complete result.
    let stat = unsafe { stat.assume_init() };
    let block_size = stat.f_frsize;
    Ok(Space {
        allocated_bytes: (stat.f_blocks as u64 - stat.f_bfree as u64) * block_size,
        total_bytes: stat.f_blocks as u64 * block_size,
        stable: false,
        observed_allocated_bytes: vec![],
    })
}
fn command(program: impl AsRef<std::ffi::OsStr>, cwd: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .current_dir(cwd)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("TWIG_TRACE", "1");
    command
}
fn checked(output: Output) -> Result<Output> {
    ensure!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}
fn git_text(cwd: &Path, args: &[&str]) -> Result<String> {
    capture(
        cwd,
        "git",
        &args.iter().map(OsString::from).collect::<Vec<_>>(),
    )
}
fn capture(cwd: &Path, program: &str, args: &[OsString]) -> Result<String> {
    Ok(
        String::from_utf8(checked(command(program, cwd).args(args).output()?)?.stdout)?
            .trim()
            .to_owned(),
    )
}
fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn save(path: &Path, suite: &Suite) -> Result<()> {
    fs::create_dir_all(path.parent().context("output needs a parent")?)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(suite)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_mode_has_no_allocation_measurement() {
        assert!(
            Filesystem::Host
                .snapshot(Path::new("/not-a-benchmark-path"))
                .unwrap()
                .is_none()
        );
        assert_eq!(allocation_delta(&None, &None), None);
    }
    #[test]
    fn preserves_negative_allocator_deltas() {
        let snapshot = |bytes| {
            Some(Space {
                allocated_bytes: bytes,
                total_bytes: 100,
                stable: true,
                observed_allocated_bytes: vec![bytes],
            })
        };
        assert_eq!(allocation_delta(&snapshot(20), &snapshot(24)), Some(-4));
    }
}
