# RFD0004 — Isolated APFS cohort benchmarks

- Status: implementation snapshot
- Author: leostera
- Date: 2026-11-15

## Decision and motivation

Compare 100 simultaneous CLI worktree creates on real small and larger committed
trees. Measure allocated filesystem growth, not summed file sizes. Ordinary host
free-space deltas include unrelated applications and du counts shared APFS blocks
incorrectly for an exclusive-space comparison.

## Contract

Each backend/repeat gets an independent 8-GiB sparse APFS image with a pinned seed
checkout. git clone --shared keeps the object store external via alternates;
only the seed's working data and worktree metadata occupy the image initially.
Never modify the original input checkout. Retain no private source contents in
raw results. Serialize labels, commit/tree IDs, sizes and metrics only.

Warm once, remove the owned warmup and staging, sync and record initial allocator
state. Start the cohort through a thread barrier. Record every invocation,
including failures, without retries. After all CLI workers join, record allocator
state; then time first status and independently verify all file contents, symlink
targets and executable bits. Sample allocation again. Change one byte per successful
checkout, require unchanged seed bytes and record the additional allocation.

statvfs reports total-minus-free blocks for the isolated filesystem, including
metadata/index/staging overhead. Stabilization observations and a stable flag are
stored. This is a filesystem allocation delta, not per-file exclusive accounting.
Detach and delete only these image-owned resources after saving each cohort.

## Alternatives, limits and validation

Host-volume df is simpler but cannot isolate this user's other active workloads.
A dedicated physical disk would reduce image-layer effects but is not available
for this first test. The image is an explicit controlled environment and results
must not be presented as universal native-host-volume throughput.

A two-worker smoke test validates creation, verification, allocator sampling and
cleanup before running 100 workers. Repeat cohorts with alternating backend order.
Failing registrations are performance/correctness findings, not samples to drop.
Private-repository results stay local unless publication is separately approved.
See [methodology](../../bench/real-repositories.md) for runnable commands.
