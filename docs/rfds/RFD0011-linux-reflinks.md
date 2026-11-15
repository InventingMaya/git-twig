# RFD0011 — Linux reflinks

- Status: implementation snapshot
- Author: leostera
- Date: 2026-11-15

## Decision and motivation

Extend the verified same-HEAD materialization path to Linux using FICLONE.
Linux users on reflink-capable filesystems get independent CoW worktrees;
unsupported filesystems continue to delegate to Git. This extends
[RFD0001](RFD0001-verified-cow-worktrees.md), without changing CLI syntax.

## Contract

For `git twig add -b feature ../feature HEAD`, enumerate the committed tree,
create private staging beside the destination, and reflink each tracked regular
file. Unlike macOS's [bulk directory clones](RFD0010-bulk-directory-cloning.md),
Linux FICLONE accepts file descriptors, not directory trees. Walk the committed
directory inventory, checking directory types without following symlinks; recreate
tracked symlinks without opening their targets. Do not enumerate ignored subtrees.

Open regular sources with O_NOFOLLOW and O_NONBLOCK, reject nonregular descriptors,
and create destinations exclusively. FICLONE either shares extents privately or
fails: never silently copy bytes or hardlink to source. Source/destination must
support the ioctl together, normally on one Btrfs or reflink-enabled XFS filesystem.
Derive checkout modes from Git executable bits and the caller's umask.

Retain Git hash verification, native registration locking, hooks and partial-state
handling. Any preparation failure falls back before registration; retained staging
can contain incomplete files. This is not a sandbox against same-user path races.
Stock Git fallback retains Git's concurrency semantics, including races between
creators targeting the same destination; the native registrar guarantee does not
extend to delegated operations.

Keep the existing APFS trace strings for existing benchmark consumers. Linux emits
`Linux reflink CoW: verified tracked files` only after successful verification.

## Testing and rollout

Run workspace formatting, Clippy, tests and release builds on Linux.
Exercise both ext4 delegation and reflink-enabled XFS in CI. Require the native
trace in the XFS job so fallback cannot silently pass native correctness tests.
Test clean status, source independence, symlinks, permissions under restrictive
umasks, ignored entries, dirty-source fallback and concurrent native registration.

## Alternatives and limitations

Delegation alone is simplest but provides no native CoW on Linux. Recursive byte
copying would add another checkout backend without delivering CoW. Recursive
directory copying would unnecessarily touch ignored trees and special files.
Per-file reflinks require syscalls per tracked file and full verification still
hashes every file. No performance or exclusive-storage claims are made here.
Binary Linux release archives and Linux allocation benchmarking are out of scope.

The ioctl definition is Linux `include/uapi/linux/fs.h`:
`FICLONE _IOW(0x94, 9, int)`; libc supplies the target-specific constant.
See also the kernel's
[ioctl_ficlonerange contract](https://www.kernel.org/doc/html/latest/filesystems/ioctl_ficlonerange.html).
