# Documentation index

Start with [ARCHITECTURE.md](../ARCHITECTURE.md) for the layout of the repository, then the rules in [AGENTS.md](../AGENTS.md).

## Start here

[Folder guide](folder-guide.md): common edits, Rust terms, naming rules and old-to-new paths.

| File | Purpose |
|---|---|
| [fidelity-contract.md](fidelity-contract.md) | Scope and what counts as established (P00) |
| [milestones.md](milestones.md) | Roadmap status and work queue |
| [unknowns.md](unknowns.md) | Open evidence and decisions |
| [supported-builds.md](research/supported-builds.md) | Builds that may be selected, with their identity |
| [source-and-release-policy.md](research/source-and-release-policy.md) | What may be read, stored and distributed (P01) |
| [source-manifest-schema.md](research/source-manifest-schema.md) | Schema v1 of the source manifest written by `source` |
| [environment.md](research/environment.md) | Measured development environment (P03) |
| [references.md](research/references.md) | Primary sources for public technical claims |
| [decisions/](decisions/) | Architecture decisions, one file per decision |

## Research notes

These describe evidence gathered from the installed build and from pinned public sources. Each note links to the task records that produced its evidence.

| File | Subject |
|---|---|
| [package-inspection.md](research/package-inspection.md) | BAR package structure |
| [image-inspection.md](research/image-inspection.md) | Image (IMGD, BC3) research |
| [model-inspection.md](research/model-inspection.md) | Model, texture and motion structure |
| [managed-inspection.md](research/managed-inspection.md) | Static managed-code research |
| [native-inspection.md](research/native-inspection.md) | Bounded static and live native research |
| [bridge-inspection.md](research/bridge-inspection.md) | Bridge qualification between native and managed evidence |
| [tool-compatibility.md](research/tool-compatibility.md) | Pinned tool and architecture audit (P05) |
| [tool-prerequisites.md](research/tool-prerequisites.md) | Prerequisites checked before each tool runs |

Ledgers (CSV): [asset-coverage.csv](research/asset-coverage.csv), [content-ledger.csv](research/content-ledger.csv), [language-ledger.csv](research/language-ledger.csv).

## Tasks

[tasks/](tasks/) holds one record per unit of work (T001 onward). Each record has an ID, a gate, its evidence locations and a verdict. The format is in AGENTS.md. [tasks/README.md](tasks/README.md) is a generated index of every record; regenerate it with `tools/New-TaskIndex.ps1`.

## Release

[release/](release/) holds the provenance note, the dependency inventory and the third-party licence files that accompany the dependencies.

## Scenarios

[scenarios/](scenarios/) holds scenario evidence. See [scenarios/README.md](scenarios/README.md) for the format.

## Superseded

- [repo-reorganization-plan.md](repo-reorganization-plan.md) is a historical proposal. T129 supersedes its code layout; use [folder-guide.md](folder-guide.md) for current paths.
