# Minimal viable product

## Core
- Lagring av repot i filsystemet
- Databas med alla commits och branches
- Hash -> Återskapa directory tree
- Skapa commit
- Skapa branch
- Koppla commit till branch
- Flytta branch (hard reset)
- Diff mellan två commits

## Frontend (CLI)
- commit (lägger till nuvarande branch)
- log (lista alla commits på nuvarande branch)
- hard-reset (flytta branch)
- branch-create
- checkout (byt branch)

# Design
- Två hashes för varje commit: Content hash / Branch hash

# Senare features
- Git compatibility
- Worktree/flera staging areas
- Staging
- Branches
- Diff viewer(s), modulär
