# git twig 🌱

Independent Git worktrees, without eagerly copying every unchanged file's data.

```sh
git twig add -b feat/parser ../feat-parser HEAD
git twig list
```

Early local prototype. Native copy-on-write creation currently supports macOS.
Other platforms and unsupported requests use ordinary `git worktree`.
**No performance or exclusive disk-space savings promised yet.**

## Try locally

Requires Rust 1.95+ and Git 2.40+.

```sh
cargo build --release --workspace
export PATH="$PWD/target/release:$PATH"
TWIG_TRACE=1 git twig add -b my-feature ../my-feature HEAD
git twig list --porcelain
```

The optimized subset is `add [-b <new-branch> | --detach] <path> [revision]`
when revision equals source HEAD. Twig clones tracked files, verifies their bytes
against that commit, and lets Git register an ordinary linked worktree.
Ignored/untracked files and local changes are **not** inherited.
Custom conversion attributes, sparse checkouts, submodules, different commits and
other options delegate to Git. A dirty source also falls back when its cloned
tracked files do not match the requested tree.

Source and destination must share a clone-capable filesystem. CoW files share
data blocks initially; writes are private. This is not a hardlink copier or
a security sandbox. File enumeration, metadata, verification and future writes
still cost time and space.

This first version retains staging directories beside destinations. TWIG_TRACE
reports them. Interrupted or failed installation may leave a registered partial
worktree; inspect it rather than retrying blindly. No automatic destructive recovery.

## Install options

Local Cargo install:

```sh
cargo install --path crates/git-twig --locked
```

After this repository is published, the same-repository Homebrew tap can be used:

```sh
brew tap leostera/git-twig https://github.com/leostera/git-twig
brew install --HEAD leostera/git-twig/git-twig
```

The tap initially builds HEAD from source. A successful tagged release updates
the formula to checksummed binary archives. Registry publishing is separate;
`cargo install git-twig` is not available until a crates.io release is published.

## Develop and measure

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
cargo run --release -p twig-bench -- --files 64 --bytes 262144 --samples 3 --concurrency 1,2,4
```

Tests and benchmarks retain synthetic fixtures under `target/`; be aware of
disk growth. See [benchmark methodology](bench/README.md),
[design contract](docs/rfds/RFD0001-verified-cow-worktrees.md), and
[release operations](docs/releases.md).

MIT licensed.
