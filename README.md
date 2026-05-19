# lit - A diff-based version control system

Repository content:

- `vcs_core`: VCS backend exposing a `Repo` interface
- `vcs_cli`: VCS frontend binary with a CLI

## Building

Build: `cargo build`

Run CLI: `cargo run -- [ARGS...]`

## Features

**NOTE: We make no guarantees about the secureness of any component of this system. Use at your own risk.**

### Diff-based revisions

Revisions (the equivalent to Git commits) store the changes from the previous revision rather than the file tree. File diffs are computed using the Myers algorithm.

### Cryptographical security

The content of revisions are cryptographically hashed and signed. When committing a revision to the repository, its location in the repository is cryptographically signed.

### Concurrency and speed

Writes/reads to the repository and database transactions are executed asynchronously using `tokio`. Diff generation utilizes multi-threading to speed up the process.

### Abstract storage interface

Both the `RepoStorage` (database) and `FileSystem` (working directory) interfaces are abstract traits, which allows users of the `vcs-core` crate and use custom implementations. `vcs-core` additionally provides two default implementations:

- `MemoryRepoStorage`/`MemoryFileSystem`: ephemeral in-memory storage, useful for testing.

- `DiskStorage`/`DiskFileSystem`: on-disk storage, offering full Unix and partial Windows compatibility.

## Future features

### Named branches/refs and ref counting

Introduce named branches to easier keep track of revisions.

Currently all objects are created and stored permanently in the database. Introduce ref counting to automatically delete unused objects.

### Diff viewer

Create a diff viewer for comparing revisions.

### Rebase with merging

Merging algorithm with interface for manual merge conflict resolving.

Rebasing command to copy revisions onto another branch.

### Git interoperability

Commands to convert revisions to/from Git commits, and to convert entire repositories.

### Optimized repository traversal

More efficient algorithm to traverse the repository. Can be used for more efficient checkouts.

### VCS server

Server application accepting pushes/pulls to upstream branches as well as rebasing remote branches.

