#import "@preview/slydst:0.1.5": *

#show: slides.with(
  title: "VCS: Version Control System",
  subtitle: "A version control system inspired by git",
  authors: "Ellinor Åhlander, Herman Hallonqvist, Jakob Puhl, Leonard Bengtsson, Wilmer Fredriksson Handler",
  subslide-numbering: "(i)",
)

#set text(lang: "sv")

#show raw: set block(fill: silver.lighten(65%), width: 100%, inset: 1em)

== Innehåll

#outline()

= Översikt av projektet
== Översikt av projektet

Version control system inspirerat av Git.

- Diff-baserat (till skillnad från Git)
- Kryptografiskt signerade och hashade commits
- Unik worktree feature, mer flexibel än Git worktrees
- Frontend i form av command line interface
- Skrivet i Rust (blazingly fast 🔥🔥🔥)

=== Namn

`lit`?

pls ge oss fler idéer 😭

= Minimum viable product
== Minimum viable product

Detta måste ingå i den grundläggande versionen av programmet:

=== Core

- Lagring av repot i filsystemet
- Databas med alla diffs, commits och branches
- Kunna återskapa directory tree från commit hash
- Skapa commit
- Skapa branch
- Koppla commit till branch
- Flytta branch (hard reset)
- Diff mellan två commits

== Minimum viable product
=== Frontend (CLI)

- stage
- unstage
- commit (lägger till nuvarande branch)
- log (lista alla commits på nuvarande branch)
- hard-reset (flytta branch)
- branch-create
- switch (byt branch)

= Andra framtida features
== Andra framtida features

=== Git interop

Konvertera repot till/från ett Git-repo

=== Remotes, networking

Gör en server-applikation som kan ta emot pushes/pulls från olika användare

== Andra framtida features
=== Workspaces

```
.                     TREE main:        -> main:
├── .vcs/             IGNORED
├── Cargo.toml                          -> main:Cargo.toml
├── docs/             TREE dev:docs/    -> dev:docs/
│   ├── mvp.md                          -> docs:docs/mvp.md
├── docs-old/         TREE main:docs/   -> main:docs/
│   └── docs.md                         -> main:docs/docs.md
└── src/                                -> main:src/
```

Explanation:
- `docs` is checked out to a separate docs branch
- `docs-old` is checked out to `docs` at `main` (could be useful for referencing old content while working on the `docs` branch)
- All other directories and files are checked out to their corresponding directories on `main` (inherited from root)

= Hur ska arbetet fördelas?
== Hur ska arbetet fördelas?

- Lagring/databas (.vcs directory)
- IO (verkställa reset, checkout-branch etc.)
- Diff creator
- Workspace-hantering (staging, index, stashing)
- Commits (metadata, signaturer etc.)
- Git-interop
- Networking/server
- CLI

== Hur ska arbetet fördelas?
=== Förslag på indelning i crates

#table(
  columns: (1fr, 3fr),
  stroke: (x: none),
  [`vcs-common`], [common types, functionality, signatures etc.],
  [`vcs-db`], [database of commits, branches, workspaces, worktrees, remotes],
  [`vcs-io`], [execute changes to working tree],
  [`vcs-diffs`], [generate diffs],
  [`vcs-core`], [core functionality: manipulate index, commits, branches],
  [`vcs-git`], [conversion to/from git repo],
  [`vcs-server`], [handle pushes/pulls to remote],
  [`vcs-porcelain`], [aggregate common actions: commit, reset, etc., utilities: log etc.],
  [`vcs-cli`], [frontend]
)

= Hur ska arbetet se ut?
== Hur ska arbetet se ut?

- Förslag: Veckovisa möten. Dag och tid TBD.
  - Kort standup, följt av ett längre möte där de som är tillgängliga jobbar tillsammans på plats
- GitHub-repo
  - Issues som TODO
  - PR:s med minst 1 review

= Vad ska ni använda för teknologi?
== Vad ska ni använda för teknologi?

Vi använder Rust.

=== (Förslag på) bibliotek

#table(
  columns: (1fr, 4fr),
  stroke: (x: none),
  [CLI], [
    - `clap`, `clap-derive`
    - `console`, `owo-colors`
    - `termtree`
  ],
  [Error handling], [
    - `thiserror`
    - `anyhow` (för frontenden)
  ],
  [Databas/lagring], [
    - `serde` (serialisering)
    - `hashbrown`
    - `bitflags`
    - ??? för databashantering
  ],
  [Concurrency], [
    - `tokio`
  ]
)

#table(
  columns: (1fr, 4fr),
  stroke: (x: none),
  [Kryptografi], [
    - `ring` (symmetric, asymmetric, signatures, random, hash)
    - `blake3`? (mycket snabbare hashing)
  ],
  [Macros], [
    - `syn`
    - `quote`
    - `proc-macro2`
  ],
  [Networking], [
    - `ssh`?
  ],
  [Övrigt/util], [
    - `itertools`
  ]
)

= Coding guidelines
== Coding guidelines

Formattering: rustfmt (med commit hooks, CI)

Bra variabelnamn. Använd rustdoc där det är lämpligt, dokumentation för moduler.

Ordentlig dokumentation av CLI:n.

