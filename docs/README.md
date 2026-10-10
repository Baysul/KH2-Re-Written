# Documentation index

Start with [ARCHITECTURE.md](../ARCHITECTURE.md) for the layout of the repository, then the rules in [AGENTS.md](../AGENTS.md).

## Start here

| File | Purpose |
|---|---|
| [fidelity-contract.md](fidelity-contract.md) | Scope and what counts as established (P00) |
| [milestones.md](milestones.md) | Roadmap status and work queue |
| [unknowns.md](unknowns.md) | Open evidence and decisions |
| [supported-builds.md](supported-builds.md) | Builds that may be selected, with their identity |
| [source-and-release-policy.md](source-and-release-policy.md) | What may be read, stored and distributed (P01) |
| [source-manifest-schema.md](source-manifest-schema.md) | Schema v1 of the source manifest written by `source_identity` |
| [environment.md](environment.md) | Measured development environment (P03) |
| [references.md](references.md) | Primary sources for public technical claims |
| [decisions/](decisions/) | Architecture decisions, one file per decision |

## Research notes

These describe evidence gathered from the installed build and from pinned public sources. Each note links to the task records that produced its evidence.

| File | Subject |
|---|---|
| [package-inspection.md](package-inspection.md) | BAR package structure |
| [image-inspection.md](image-inspection.md) | Image (IMGD, BC3) research |
| [model-inspection.md](model-inspection.md) | Model, texture and motion structure |
| [managed-inspection.md](managed-inspection.md) | Static managed-code research |
| [native-inspection.md](native-inspection.md) | Bounded static and live native research |
| [bridge-inspection.md](bridge-inspection.md) | Bridge qualification between native and managed evidence |
| [tool-compatibility.md](tool-compatibility.md) | Pinned tool and architecture audit (P05) |
| [tool-prerequisites.md](tool-prerequisites.md) | Prerequisites checked before each tool runs |

Ledgers (CSV): [asset-coverage.csv](asset-coverage.csv), [content-ledger.csv](content-ledger.csv), [language-ledger.csv](language-ledger.csv).

## Tasks

[tasks/](tasks/) holds one record per unit of work (T001 onward). Each record has an ID, a gate, its evidence locations and a verdict. The format is in AGENTS.md.

## Release

[release/](release/) holds the provenance note, the dependency inventory and the third-party licence files that accompany the dependencies.

## Scenarios

[scenarios/](scenarios/) holds scenario evidence. See [scenarios/README.md](scenarios/README.md) for the format.

## Superseded

- [architecture.md](architecture.md) describes the foundation state with three crates and no decoder. [ARCHITECTURE.md](../ARCHITECTURE.md) replaces it.
- [repo-reorganization-plan.md](repo-reorganization-plan.md) is the proposal for the moves and deletions in progress. It is not a record of what changed.
