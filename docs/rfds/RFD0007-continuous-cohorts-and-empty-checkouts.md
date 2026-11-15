# RFD0007 — Continuous cohorts and empty checkouts

- Status: implementation snapshot
- Author: leostera
- Date: 2026-11-15

## Decision

The person requested waiting for a complete hundred-way kernel comparison after
the earlier budget-stopped experiments. Restart both backends sequentially with
no time cutoff, while retaining the approved scratch-space ceiling and safeguards.
Keep previous partial runs as partial data; never promote them into comparisons.

Each completed CLI writes an atomic creation-progress checkpoint outside the
disposable image. Checkpoints explicitly say verification is pending. Full
content/mode/symlink verification still waits until all create workers finish,
so verification does not compete with measured creation. Only the final cohort
records establish verified results. Progress serialization is observer overhead
included in cohort wall time, not in per-operation CLI latency.

## Empty input and table contract

An empty repository fixture has one empty initial commit and zero tracked files.
It is not an unborn repository. Worktrees remain ordinary linked worktrees.
If no regular file exists, the private-write test is not applicable: record
privateWriteTested=false and no write-allocation measurement, then require a clean
seed status. Never fabricate a file merely to make an empty-worktree benchmark run.

README repositories are columns and metrics are rows. Complete entries retain all
publication gates from RFD0006. Explicitly incomplete entries may show independently
verified individual backend results, but missing values are labelled budget-stopped
and paired counts are zero. Allocation may be unmeasured, never implicitly zero.
