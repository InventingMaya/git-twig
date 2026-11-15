# git twig 🌱

Independent Git worktrees, without eagerly copying every unchanged file's data.

```sh
git twig add -b feat/parser ../feat-parser HEAD
git twig list
```

Early local prototype. Native copy-on-write creation currently supports macOS.
Other platforms and unsupported requests use ordinary `git worktree`.
Performance depends on the workload; tiny repositories can be slower.

## Local benchmarks

<!-- twig-bench:start -->

<!-- Generated from bench/results/readme.json by twig-readme-table. Do not edit numbers. -->

| metric | [new repo (empty commit)](bench/results/2026-11-15-real/empty-host.json) | [git-twig](bench/results/2026-11-15-real/host.json) | [Axo](bench/results/2026-11-15-real/host.json) | [Linux kernel (10 sequential)](bench/results/2026-11-15-real/kernel-sequential10.json) |
|---|---|---|---|---|
| Git time | 1.79 s | 4.90 s | 279.81 s | 201.93 s |
| Twig time | 8.02 s | 10.13 s | 138.79 s | 365.14 s |
| Git space | not measured | 53.0 MiB | 3100.2–3101.2 MiB | 18762.4–18762.8 MiB |
| Twig space | not measured | 7.1 MiB | 164.9 MiB | 573.0 MiB |

Time is batch creation; space is additional APFS allocation after first status. Batch sizes differ by repository; see setup below.

<details>
<summary>Setup, verification and detailed measurements</summary>

| metric | [new repo (empty commit)](bench/results/2026-11-15-real/empty-host.json) | [git-twig](bench/results/2026-11-15-real/host.json) | [Axo](bench/results/2026-11-15-real/host.json) | [Linux kernel (10 sequential)](bench/results/2026-11-15-real/kernel-sequential10.json) |
|---|---|---|---|---|
| tracked files | 0 | 45 | 2083 | 97371 |
| worktrees per run | 100 | 100 | 100 | 10 |
| simultaneous creates | 100 | 100 | 100 | 1 |
| timing filesystem | host APFS | host APFS | host APFS | case-sensitive APFS image |
| Git create | 1.79 s | 4.90 s | 279.81 s | 201.93 s |
| Twig create | 8.02 s | 10.13 s | 138.79 s | 365.14 s |
| Git first status (median) | 0.01 s | 0.02 s | 0.04 s | 1.78 s |
| Twig first status (median) | 0.01 s | 0.02 s | 0.32 s | 16.34 s |
| Git allocation | not measured | 53.0 MiB | 3100.2–3101.2 MiB | 18762.4–18762.8 MiB |
| Twig allocation | not measured | 7.1 MiB | 164.9 MiB | 573.0 MiB |
| paired timing rounds | 3 | 3 | 1 | 1 |
| Git / Twig failed attempts | 0 / 0 | 2 / 0 | 0 / 0 | 0 / 0 |
| status | completed | completed | completed | completed |

Creation is wall time for the entire batch at the recorded concurrency, excluding first status. Values are medians of the recorded timing rounds; one round is an observation, not a stable estimate. Allocation is additional **filesystem-allocated MiB after first status**, including indexes, retained staging and APFS metadata, measured separately on isolated images. Ranges retain observed allocator variation; they are not confidence intervals. Missing measurements are not zeros. Never infer exclusive APFS allocation from logical size or `du`.

- These are local, warm-cache results on a shared Mac, not universal speed guarantees. Input commits and hardware are recorded in each raw run.
- Only tracked committed files are copied. Ignored dependencies and build artifacts are excluded.
- For host-APFS rows, allocation was measured separately on isolated images using the same pinned trees and candidate binary. The kernel row uses matched case-sensitive APFS images for both timing and allocation. Round counts are shown.
- Plain Git's nonzero failure count is an observed concurrent commondir-registration race. Failed attempts remain in the timing samples; a fast failed batch is not a complete successful cohort.
- The new-repo fixture contains zero tracked files and one empty initial commit, not an unborn HEAD. Its private-write probe is not applicable and allocation was not measured on the shared host filesystem.
- The Linux kernel row is one paired round of ten sequential creates per backend (maximum one in flight), not a hundred-way result. Its case-sensitive APFS image environment differs from the host-APFS timing rows; do not compare absolute times across workloads as if environments were identical.
- Earlier kernel parallel attempts were [budget-stopped](bench/results/2026-11-15-real/kernel-n100-git-event.json) or user-interrupted ([Git](bench/results/2026-11-15-real/kernel-git-user-interrupted.json), [Twig](bench/results/2026-11-15-real/kernel-twig-user-interrupted.json)). Those artifacts are retained but their cutoff times are not completed timings or speedup baselines.

[Methodology](bench/real-repositories.md) · [raw run index](bench/results/readme.json). First-status latency, p95/p99, errors, source revisions, candidate SHA-256, platform and full samples are in the linked raw runs.

</details>

<!-- twig-bench:end -->

## Try locally

Requires Rust 1.95+ and Git 2.40+.

```sh
cargo build --release --workspace
export PATH="$PWD/target/release:$PATH"
TWIG_TRACE=1 git twig add -b my-feature ../my-feature HEAD
git twig list --porcelain
```

The optimized subset is `add [-b <new-branch> | --detach] <path> [revision]`
when revision equals source HEAD. Twig clones tracked files, verifies their bytes
against that commit, and lets Git register an ordinary linked worktree.
Ignored/untracked files and local changes are **not** inherited.
Custom conversion attributes, sparse checkouts, submodules, different commits and
other options delegate to Git. A dirty source also falls back when its cloned
tracked files do not match the requested tree.

Source and destination must share a clone-capable filesystem. CoW files share
data blocks initially; writes are private. This is not a hardlink copier or
a security sandbox. File enumeration, metadata, verification and future writes
still cost time and space.

This first version retains staging directories beside destinations. TWIG_TRACE
reports them. Interrupted or failed installation may leave a registered partial
worktree; inspect it rather than retrying blindly. No automatic destructive recovery.

## Install options

Local Cargo install:

```sh
cargo install --path crates/git-twig --locked
```

Install from the same-repository Homebrew tap:

```sh
brew tap leostera/git-twig https://github.com/leostera/git-twig
brew install --HEAD leostera/git-twig/git-twig
```

The tap initially builds HEAD from source. A successful tagged release updates
the formula to checksummed binary archives. Registry publishing is separate;
`cargo install git-twig` is not available until a crates.io release is published.

## Develop and measure

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
cargo run --release -p twig-bench -- --files 64 --bytes 262144 --samples 3 --concurrency 1,2,4
```

Tests and benchmarks retain synthetic fixtures under `target/`; be aware of
disk growth. See [benchmark methodology](bench/README.md),
[design contract](docs/rfds/RFD0001-verified-cow-worktrees.md), and
[release operations](docs/releases.md).

MIT licensed.
