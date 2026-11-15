---
name: git-twig
description: Create and manage ordinary linked Git worktrees with git twig, using APFS copy-on-write to reduce disk use. Use when a task needs an isolated checkout or branch.
---

# git twig

Use `git twig` in place of `git worktree`. The resulting checkout is a normal
linked worktree: use ordinary Git commands inside it.

## Create a workspace

From an existing checkout:

```sh
git twig add -b agent/my-task ../my-task HEAD
cd ../my-task
```

For a detached checkout:

```sh
git twig add --detach ../experiment HEAD
```

Use an explicit `-b` or `--detach` to enable the CoW path. Choose a new destination;
never overwrite an existing directory. Run tests and make commits in the new
worktree as usual. `git twig list --porcelain` lists worktrees.

## Know what is copied

- Only committed tracked files are included. Local edits and untracked/ignored
  files are excluded; tracked secrets are copied like any other tracked file.
  Install dependencies when needed.
- Native CoW needs macOS/APFS and the same clone-capable filesystem. Creating at
  the source's HEAD is the optimized case. Other requests use stock Git.
- CoW shares file data, not writes. It is not a sandbox and is not always faster.
- To inspect backend selection, set `TWIG_TRACE=1` on the create command.

## Cleanup safely

Ask before removing a worktree or discarding changes. Check `git status` first;
never force removal, prune registrations or delete staging directories blindly.
After approval, use `git twig remove <path>`.

This is a prototype. Staging directories are currently retained. On failure,
inspect `git twig list --porcelain` and the paths reported by `TWIG_TRACE`; a partial
worktree may already be registered. Do not retry into it or delete it automatically.
