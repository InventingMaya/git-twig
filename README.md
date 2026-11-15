# git twig 🌱

Git worktrees that share file data on APFS instead of copying it.

## Install

```sh
brew tap leostera/git-twig https://github.com/leostera/git-twig
brew install --HEAD leostera/git-twig/git-twig
```

## Use

```sh
git twig add -b my-feature ../my-feature
git twig list
```

For coding agents: `git twig --skill` prints the bundled skill.

macOS/APFS. Creates ordinary linked worktrees from committed files; local changes
and ignored files aren't copied. Unsupported options use `git worktree`.

## Benchmarks

<!-- twig-bench:start -->

<!-- Generated from bench/results/readme.json by twig-readme-table. Do not edit numbers. -->

| Repo | `git worktree` | `git twig` | Improvement |
|---|---|---|---|
| [new repo (empty commit)](bench/results/2026-11-15-real/empty-directory-host.json) | 1.77s / 3.3 MiB | 5.78s / 3.4 MiB | 3.26× slower / 1.0× more space |
| [git-twig](bench/results/2026-11-15-real/host.json)† | 4.90s / 53.0 MiB | 10.13s / 7.1 MiB | 2.07× slower / **7.5× less space** |
| [Axo](bench/results/2026-11-15-real/host.json) | 4m40s / 3.0 GiB | 2m19s / 165 MiB | 2.02× faster / **18.8× less space** |
| [Linux kernel (10 sequential)](bench/results/2026-11-15-real/kernel-directory10.json) | 3m15s / 18.3 GiB | 4m32s / 549 MiB | 1.40× slower / **34.2× less space** |

Each cell is **creation time / additional disk space**. Batch sizes differ; creation excludes first status. † Includes failed Git attempts. [Setup and raw results](bench/results/readme-details.md).

<!-- twig-bench:end -->

[MIT](LICENSE)
