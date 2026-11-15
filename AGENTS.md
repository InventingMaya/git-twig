# git-twig

Rust workspace. Keep the CLI thin; materialization belongs in twig-core.
Integration: direct to main, after local tests. Do not publish or tag releases without confirmation.
Never claim full Git worktree compatibility; unsupported operations are delegated.
Run cargo fmt --all --check, cargo clippy --workspace --all-targets -- -D warnings,
and cargo test --workspace before committing.
Benchmarks use release binaries, compare actual CLI operations, retain versioned JSON,
and generate charts from those files. Never claim exclusive APFS allocation from du.
Keep user source and existing destinations untouched. Never hardlink files to source.
Synthetic test/benchmark fixtures are retained under target/ until explicitly removed.
