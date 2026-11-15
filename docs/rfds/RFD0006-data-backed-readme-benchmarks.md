# RFD0006 — Data-backed README benchmarks

- Status: implementation snapshot
- Author: leostera
- Date: 2026-11-15

## Decision

Generate the README benchmark block from committed raw runs selected by
bench/results/readme.json. Do not hand-edit performance numbers or fill missing
cohorts with forecasts. CI runs the generator in --check mode.

## Contract and validation

Each selected row requires completed paired backend cohorts, identical pinned
input trees/commits and candidate binary hashes across timing/allocation runs,
and unchanged seed bytes. A command that reports success but fails independent
content verification blocks publication. Native Twig must have used CoW, not
delegated to Git. Git registration failures remain visible alongside timings;
a failed fast attempt is not a successful checkout.

Show cohort size, tracked file count, timing filesystem, absolute cohort wall
times, allocation, sample counts and failure counts. Creation excludes first
status, which remains separately recorded in raw data. Host creation timings and
isolated-image allocation are distinct measurements, not one fictitious cohort.
Allocation intervals retain observed filesystem allocator variation.

Linux-kernel case collisions require case-sensitive APFS images on this Mac.
The pinned kernel tree is not faithfully representable on case-insensitive APFS.
Image format and capacity are explicit run configuration. Kernel image timings
must be labelled separately from native host-APFS timings.

The initial kernel experiment has a person-requested thirty-minute total budget.
Ramp only while the remaining budget permits; terminate owned jobs and detach
owned images at the deadline. Publish only completed measurements, recording
unattempted sizes and any budget stops. Do not imply that a smaller kernel cohort
is a measured hundred-worktree cohort.

## Privacy and limitations

Publication of the existing private-workload performance metrics was explicitly
approved; raw files contain metrics/provenance, not private source contents or
input filesystem paths. Review artifacts before committing. Private source is
not made public to support a benchmark.

These are local shared-Mac measurements. Single rounds are observations, not
stable tail estimates, and environments differ by labelled row. No universal
speedup or exclusive per-file APFS allocation claim follows from this table.
