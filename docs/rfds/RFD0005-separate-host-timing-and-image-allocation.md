# RFD0005 — Separate host timing from image allocation

- Status: implementation snapshot
- Author: leostera
- Date: 2026-11-15
- Updates: RFD0004 benchmark methodology

## Decision and motivation

Keep isolated APFS images for allocation accounting, but add a direct-host mode
for representative creation timing. A high-concurrency image run can bottleneck
on disk-image I/O and metadata, so its wall time must not stand in for normal
host-APFS latency.

## Contract

--filesystem isolated-image retains RFD0004 behavior. --filesystem host creates
fresh cohort directories directly on the host filesystem, retains warmups and
fixtures, and never deletes them. Both backends have the same retained warmup.
Host runs do not sync or claim allocated-space growth: space snapshots and deltas
are absent (null), not misleading zeros. The report renders them as unmeasured.

Schema v2 records filesystem mode. The plotter also reads v1 image runs.
--revision label=COMMIT pins each input to the exact same full commit OID across
image and host runs even when the original checkout's HEAD subsequently moves.
Source repositories remain read-only; the seed and object alternates are unchanged.

## Interpretation and alternatives

Report host timings and image allocation as separate measurements with explicit
source/candidate provenance. Never combine them into a fictitious single cohort.
Publishing image timings is fine if labelled as such, but they do not establish
native-host speed. Host free-space deltas remain too noisy to claim exclusive
CoW savings while other apps run. A dedicated physical volume would be preferable
but is outside this first local experiment.
