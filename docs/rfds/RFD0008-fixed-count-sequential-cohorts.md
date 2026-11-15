# RFD0008 — Fixed-count sequential cohorts

- Status: implementation snapshot
- Author: leostera
- Date: 2026-11-15

## Decision

Run ten kernel worktree creates per backend, sequentially, as requested after the
large parallel experiments were interrupted. Separate total work from concurrency:
--worktrees 10 --concurrency 1 --repeats 1. No elapsed-time cutoff is implied.

A bounded worker pool takes unique operation IDs from an atomic counter. Each
worker waits for its current CLI to return before taking the next operation.
One worker therefore means no overlapping creates, not ten simultaneously started
commands. Existing commands that omit --worktrees retain a cohort count equal to
concurrency.

## Evidence and reporting

Record worktreesPerCohort and concurrency separately. Every operation has monotonic
createStartedAtMs/createFinishedAtMs relative to cohort start, in addition to
latency. Sequential runs assert non-overlapping intervals, and publication checks
the intervals again. A small real-CLI smoke test exercises both backends with
three sequential worktrees and validates all contents.

Creation checkpoints retain each returned CLI outcome immediately. These are not
independent verification: the full content/mode/symlink and source-isolation checks
still run after the entire creation queue drains, outside measured creation time.
Warmup and source materialization remain outside the creation timer.

Both backends use fresh matched case-sensitive APFS images, the same pinned kernel
tree, frozen candidate/Git binaries, and the same sequential harness. Retain the
previous interrupted experiments as partial data; do not turn cutoffs into
completion times or speedup ratios. Display total worktree count and simultaneous
creates as distinct README rows.
