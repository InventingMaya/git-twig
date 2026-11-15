# Local CLI benchmarks

Question: does verified APFS cloning reduce real worktree creation **plus first
status** latency versus the installed stock Git? Numeric correctness target:
zero wrong checkouts and zero source mutations. Speed is an open measurement,
not a promise.

```sh
cargo build --release --workspace
cargo run --release -p twig-bench -- --files 64 --bytes 262144 --samples 3 --concurrency 1,2,4 --output bench/results/mixed.json
cargo run --release -p twig-bench --bin twig-plot -- bench/results/mixed.json
```

Run from the repository root with the release git-twig already built.
The harness creates deterministic pseudo-random, committed fixtures, warms each
backend once, alternates backend order, drives actual CLI subprocesses and retains
all fixtures under target/bench-fixtures. No shell command string interpolation.
Git global/system configuration is disabled for comparability.

Each raw JSON run has schemaVersion, timestamp, source commit/dirty state, Git,
Rust and candidate versions, platform, CPU, full workload configuration and raw
per-operation timings/errors/correctness counters. It is saved after each point.
Report p50/p95/p99/max from those samples; small samples do not support strong
tail claims. Plot files and summary tables are generated from raw JSON only.

First and warm status are timed before full byte verification. Each created file
is verified, then an in-place write must leave the source unchanged and Git must
detect the edit. On macOS the candidate must actually select CoW or the run fails.
Batch throughput includes status and correctness overhead; do not confuse it with
creation-only throughput. Warmup/fixture generation are not measured.
Hardware is shared with other local apps; no cold-cache, production or exclusive
physical-storage claim.

Suggested suites:
- byte-heavy: --files 16 --bytes 16777216
- file-heavy: --files 5000 --bytes 1024
- mixed: --files 64 --bytes 262144
- ramp: repeat profiles with --concurrency 1,2,4,8,16,32 until a gate fails
  or disk/host budget requires a stop; record the stopping reason.

Retaining worktrees makes later operations share a growing Git worktree registry.
Order alternates to reduce this bias, not eliminate it. Fixtures may consume many
GiB, especially stock Git copies. Inspect free space first; cleanup is an explicit
separate operation. du and st_blocks cannot establish exclusive APFS usage.

Commit the harness before a publishable run. Commit raw JSON and generated SVG
supporting public claims; fixture data never belongs in Git. A dirty run is
exploratory only. Failed runs remain data, never silently disappear.
