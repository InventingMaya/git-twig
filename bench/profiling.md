# Release overhead profiling

Question and capture contract: [RFD0009](../docs/rfds/RFD0009-profile-release-overheads.md).
This is an instrumented diagnostic, not a replacement for the uninstrumented
[README results](../README.md#local-benchmarks).

```sh
cargo build --release --workspace
# Run from the repository root; output must not already exist.
target/release/twig-profile \
  --source target/kernel-input.git \
  --revision 23c62304dde29abe92013d85910d03a77dada35d \
  --candidate target/release/git-twig \
  --harness target/release/twig-real-bench \
  --repeats 3 --output target/profiles/kernel-release
```

The Git wrapper is copied into the output directory, and forwards to the resolved
installed Git with its pinned exec-path. Keep candidate and harness binaries frozen
for the entire capture; do not run other benchmarks or builds alongside it.

- `results.json`: real CLI outcomes and independent correctness checks.
- `git-calls/*.json`: each Git invocation, caller/child PID, monotonic duration,
  arguments and staging-index/work-tree environment. Expected config misses may
  return status 1; do not classify these as failed creates.
- `trace2/`: native Git Trace2 events, including setup/warmup and verification.
- `samples/`: 5 ms stack samples for processes observed alive for at least 500 ms;
  corresponding stderr and final collector log expose tool failures.
- `state.json`: progress and observed descendant processes; `run.log`: harness log.

Only sample candidate and real Git processes associated with a measured `w-N`
worktree, not the load generator. The process inventory polls every 250 ms. Stack
samples include waits, not just CPU execution. Git wrapper startup and record
writes contribute observer overhead. Correlate direct-child Git records with the
candidate PID; distinguish `GIT_INDEX_FILE` staging verification from the installed
worktree's first status. Do not sum nested Git elapsed intervals twice.

Images are created/detached/removed by the existing harness, serially, with a hard
32 GiB capacity per image. Before starting, ensure at least 128 GiB free; an operator
supervisor must retain that free-space safeguard during the run. Interrupted
captures may leave owned images requiring inspection and cleanup. Original source
repositories are read-only inputs. Retain failures and interrupted data honestly.

Local raw Trace2 and sample files may include absolute paths. Before committing
profiles, replace project/home paths with placeholders, inspect for unrelated data,
record tool versions and executable SHA-256s, and generate numeric views from the
versioned data. Never publish private source contents or unreviewed local profiles.
