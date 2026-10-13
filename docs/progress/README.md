# KH2 Final Mix progress

Baseline: Steam app 2552430 build 15194255 candidate - supported profile unqualified.

| Measure | Accepted / tracked | Percentage |
|---|---:|---:|
| Complete functions decompiled and checked | 0 / 4 (inventory incomplete) | Unmeasured |
| Required behavior scenarios verified 1:1 | 0 / 0 (inventory incomplete) | Unmeasured |
| Final roadmap gates accepted | 0 / 32 | 0.00% |

A PASS counts one reviewed unit. All other statuses count zero. Unknown required units stay in the denominator; an incomplete inventory suppresses the overall percentage. Functions and behavior scenarios have separate denominators and are never averaged into a whole-game score.

## Roadmap acceptance

| Gate | Objective | Verdict | Evidence |
|---|---|---|---|
| P00 | Scope and fidelity contract | PARTIAL | [record](../../docs/milestones.md) |
| P01 | Installed-source and release boundary | PARTIAL | [record](../../docs/milestones.md) |
| P02 | Installed edition/build identity | PARTIAL | [record](../../docs/milestones.md) |
| P03 | Windows/Rust/Bevy environment | PARTIAL | [record](../../docs/milestones.md) |
| P04 | Durable repository/evidence | PARTIAL | [record](../../docs/milestones.md) |
| P05 | Pinned tool/license audit | PARTIAL | [record](../../docs/milestones.md) |
| P06 | Native folder picker and source validation | PARTIAL | [record](../../docs/milestones.md) |
| P07 | Packages/archives/dependencies | PARTIAL | [record](../../docs/milestones.md) |
| P08 | Read-only importer/source-bound cache | PARTIAL | [record](../../docs/milestones.md) |
| P09 | Reference capture/replay harness | PARTIAL | [record](../../docs/milestones.md) |
| P10 | Installed executable/runtime research | PARTIAL | [record](../../docs/milestones.md) |
| P11 | Authoritative simulation boundary | NOT RUN | [record](../../docs/milestones.md) |
| P12 | Traceable decoded representation | PARTIAL | [record](../../docs/milestones.md) |
| P13 | Asset/animation inspection app | PARTIAL | [record](../../docs/milestones.md) |
| P14 | PC rendering/presentation | PARTIAL | [record](../../docs/milestones.md) |
| P15 | Animation/motion events | PARTIAL | [record](../../docs/milestones.md) |
| P16 | Rooms/collision/streaming/actors | PARTIAL | [record](../../docs/milestones.md) |
| P17 | Input/time/RNG/determinism | NOT RUN | [record](../../docs/milestones.md) |
| P18 | Movement/camera/player states | PARTIAL | [record](../../docs/milestones.md) |
| P19 | Combat/enemies/reactions | PARTIAL | [record](../../docs/milestones.md) |
| P20 | Scripts/native services | PARTIAL | [record](../../docs/milestones.md) |
| P21 | Complete playable slice | PARTIAL | [record](../../docs/milestones.md) |
| P22 | Progression/party/inventory/systems | PARTIAL | [record](../../docs/milestones.md) |
| P23 | Native saves/safe vendor import | NOT RUN | [record](../../docs/milestones.md) |
| P24 | Story/cutscenes/transitions | PARTIAL | [record](../../docs/milestones.md) |
| P25 | Menus/HUD/settings/EN/JA/fonts | PARTIAL | [record](../../docs/milestones.md) |
| P26 | Audio/music/synchronization | PARTIAL | [record](../../docs/milestones.md) |
| P27 | Every world/encounter/mode | NOT RUN | [record](../../docs/milestones.md) |
| P28 | Campaign and both text options | NOT RUN | [record](../../docs/milestones.md) |
| P29 | Steam changes/recovery | NOT RUN | [record](../../docs/milestones.md) |
| P30 | Performance/Windows reliability | NOT RUN | [record](../../docs/milestones.md) |
| P31 | Windows package/release qualification | NOT RUN | [record](../../docs/milestones.md) |

## Complete functions decompiled and checked

| Unit | Scope | Verdict | Evidence |
|---|---|---|---|
| CIL-T010-01 | T010 managed method selection 1 - historical decompile, equivalence unproved | PARTIAL | [record](../../docs/tasks/T010-managed-reader.md) |
| CIL-T010-02 | T010 managed method selection 2 - historical decompile, equivalence unproved | PARTIAL | [record](../../docs/tasks/T010-managed-reader.md) |
| CIL-T010-03 | T010 managed method selection 3 - historical decompile, equivalence unproved | PARTIAL | [record](../../docs/tasks/T010-managed-reader.md) |
| CIL-T010-04 | T010 managed method selection 4 - historical decompile, equivalence unproved | PARTIAL | [record](../../docs/tasks/T010-managed-reader.md) |

Accepted share of the tracked subset: 0.00%. This is not a whole-game percentage.

## Required behavior scenarios verified 1:1

No complete accepted units have been entered; the full denominator remains unqualified. This does not mean no research or implementation exists.

Accepted share of the tracked subset: Unmeasured. This is not a whole-game percentage.

## Maintaining the ledger

Edit `docs/progress/coverage.json` when a reviewed evidence record changes a verdict. Add one opaque ID per complete logical function or required behavior scenario, including untested and blocked units. Keep IDs stable and retain failed units. Do not enter addresses, disassembly, pseudocode, source hashes, personal paths or private evidence files.

For decompilation, PASS requires qualified complete logical function bounds and a reviewed source-bound decoding/decompilation check. An interior slice or byte identity alone is PARTIAL. For fidelity, PASS requires a controlled installed/rewrite comparison under the fidelity contract with no unresolved required discrepancy. Roadmap PASS requires the final gate acceptance, not a task's local build/test PASS.

Mark an inventory complete only after its public inventory evidence documents full KH2FM scope, shared dependencies, deduplication, exclusions and remaining unknown units. Every PASS links to its public acceptance record. The generator validates structure and links; evidence review still determines whether the claim is justified.

Run `python tools/progress.py` to refresh the README, this report and `summary.json`. Run `python tools/progress.py --check` to detect stale outputs. JSON null means unmeasured, never zero. The output is deterministic and contains only public ledger labels, statuses, counts and a ledger digest. It never reads Steam files or private research.

## GitHub refresh

The progress workflow tests and generates on each main-branch push, daily at 13:17 UTC and on manual dispatch. Pull requests validate and preview without write permissions. Only changed generated files are committed; unchanged schedules create no commits. A concurrent main update is rejected by normal fast-forward push; the next push or scheduled run regenerates from the new head. Branch protection may require a maintainer to allow the bot's documentation updates.

GitHub schedules run on the default branch and may be delayed: [workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax). The built-in GITHUB_TOKEN prevents generated commits from recursively triggering more push workflows: [triggering workflows](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow). Refreshing reports cannot discover private evidence or advance a verdict without a reviewed ledger update.
