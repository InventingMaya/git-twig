# RFD0002 — Separate benchmark generation from measured load

- Status: implementation snapshot
- Author: leostera
- Date: 2026-11-15
- Supersedes: the benchmark timing arrangement in RFD0001, not the Twig protocol

## Decision and motivation

All create/first-status/warm-status workers must finish before any full-content
verification or private-write test starts. Schema v1 let one worker's synthetic
byte generation overlap another worker's creation. Its batch throughput included
verification work. This can measure the harness rather than the system and makes
v1 exploratory, not evidence for headline speed claims.

## Contract

Schema v2 retains the same operation metrics but measures batch wall time only
around concurrent create/status workers. Correctness checks run after joining all
workers, outside that batch interval. Write verification and modified status are
still recorded. Failed operations remain in raw data and block conclusions.
Candidate binary SHA-256 and fixture Git tree ID supplement source commit, dirty
state, versions, platform and seed. The plotter supports both schema versions;
raw v1 runs are preserved unchanged for traceability.

## Alternatives and limitations

A separate load-generator machine would avoid local CPU competition but is not
appropriate for a filesystem operation on this Mac. Removing verification would
make the benchmark cheaper but unsafe. Separating phases retains correctness
while avoiding our known contention source. Other local apps and Git's growing
worktree registry still introduce noise. Concurrency ramps stop at an explicit
host/disk budget; no claim of maximum capacity follows from that stop.

## Validation

Run the actual release CLI against the installed Git, repeat profiles, alternate
backend order, require native CoW selection on macOS, preserve every raw point and
generate charts/tables from those files. Zero wrong checkouts/source mutations is
the acceptance gate. Byte-heavy, file-heavy and mixed profiles remain distinct.
