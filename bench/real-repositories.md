# Real repositories: concurrent creation and APFS allocation

This suite measures a cohort of real CLI processes, not an internal copier.
Run on macOS from this repository root, with permission to mount and delete
disposable disk images. Original repository paths are read-only inputs.

```sh
cargo build --release --workspace
target/release/twig-real-bench \
  --repo small=/absolute/small/repo \
  --repo large=/absolute/large/repo \
  --concurrency 100 --repeats 3 \
  --output target/real-results/100-way.json
target/release/twig-real-plot target/real-results/100-way.json
```

Each image-backed backend/repeat gets a fresh sparse APFS image (8 GiB by default;
configure capacity with `--image-gib`). Use `--filesystem case-sensitive-image`
for trees with case-colliding paths, such as the Linux kernel. Never benchmark such
a tree on case-insensitive APFS and silently accept an incomplete checkout. A seed clone uses
shared Git objects through alternates into the original repository, then checks
out the pinned commit inside the image. The seed and all created worktrees share
the image's APFS filesystem; history remains external and identical for both.
No source hooks/global Git config are inherited. Ignored/untracked dependencies
and build artifacts are not copied.

A barrier starts all requested CLI workers together. The harness records cohort
wall time, per-attempt create latency, exit/error details and native-backend
selection. It does not retry failed registrations. First status and full
byte/symlink/executable verification happen after all create workers join.
Per-operation first-status latency is recorded separately; the aggregate
status-and-verification time includes verification and is not pure status time.

## Direct host timing

Use `--filesystem host` to measure normal host-APFS creation without the disk-image
layer. It retains all owned fixture directories (including warmups), never deletes
them, never forces sync, and reports space as unmeasured. Use `--revision label=OID`
after `--repo` to match the exact pinned commits from an earlier image run.
Compare both backends under the same mode; do not present host timings and image
space as if they came from one cohort. Schema v2 records the mode; the report also
supports v1 image results.

## Space method

Sum neither logical sizes nor st_blocks. After sync, sample statvfs allocation on
the isolated image until stable (or mark unstable). Subtract pre-cohort allocated
space from allocation after creates and after status. This measures the entire
filesystem delta, including working files, indexes, staging and APFS metadata.
No unrelated user's apps should write to this image; Spotlight indexing is
disabled using a root .metadata_never_index marker.

Then change one byte in one regular file per successful worktree, measure further
allocation and verify the seed file remains unchanged. This demonstrates write
isolation and records a small-edit allocation budget without claiming every file's
exclusive block ownership is observable. Negative deltas are retained, not clamped.
The sparse image's host-file allocation is not the primary metric.

Warmup worktrees and their owned staging paths are removed before the initial
space snapshot. After saving each cohort, detach and delete its image. On failure,
inspect logs and hdiutil info for any remaining mounted image; never force-detach
an unrelated volume. Empty fixture directories may remain under target/real-images.

## Interpretation

A fast cohort with failed operations did not create the requested number of
worktrees successfully. Report success counts, errors and content verification
alongside speed and allocation. Small committed trees with huge ignored output
are not large checkout workloads. Source commit/tree IDs, file counts and logical
bytes distinguish those cases.

Backend order alternates by repeat. Caches remain warm; filesystem image layering
is controlled but differs from the normal host APFS volume. Capture raw results
before making claims. Private-repository results stay in target/ by default;
only publish with separate permission. Keep versioned private copies when needed.
