# T172 — Read-only Epic source probes

Owner: coding agent. Gates: P00/P02/P03/P06 evidence gathering only; no final gate advances. Prerequisites: fidelity contract, milestones, unknowns, folder guide, T171, and the user's selected Epic KH2FM installation reported download-complete. Scope: inspect that completed candidate read-only and identify concrete reuse/blockers; not implement native Linux/Epic support, change the supported-source contract, or inspect other collection games.

## Build and source identity

Audited rewrite commit: `554d3bf18d3b`; Linux x86_64; Rust/Cargo 1.99.0; Bevy pin 0.19.1. Selected candidate **E01** is bound to the private `inventory_preflight.json` and matching `inventory_postcheck.json` described below. Its vendor build identity, pristine status and behavioral compatibility remain unknown; it is **quarantined/unqualified**. No current Steam installation was directly compared byte-for-byte. A matching record count is not content equivalence.

No installed executable was launched, source file modified, vendor save accessed, purchase made or installation repaired/updated. No game bytes, executable extracts, addresses, source hashes, personal paths, saves or captures were added to Git. Runtime code, tests, Cargo manifests/lockfile, fidelity contract and coverage ledger are unchanged. T171's earlier absence of supplied Epic files remains a historical observation rather than being overwritten.

## Steps and exact evidence locations

`PRIVATE_EVIDENCE` denotes `$HOME/.local/share/kh2-rewritten/research/T172_epic_source_probe/` on the inspected host; this notation intentionally omits the user's absolute home/source path. All actual source identities and derived private details remain there, outside both installation and checkout.

1. Re-read the required scope/roadmap/unknown/task records. Independently trace the scanner, source binding, archive storage, name lookup and executable/save boundaries. Public source citations and the reviewed independent research are in `docs/research/epic_reader_boundaries.md`. Its agent transport failed after writing the file, so Main read and checked the actual report.
2. Restrict selected source reads to the KH2FM executable and six `kh2_*.hed`/`.pkg` pairs. Observe all pairs under `Image/en`, not the current `Image/dt` hardcoding (`validation/src/lib.rs:228-239`, `source/data/src/path_lookup.rs:9-17`). Do not rename/link source files to force the scanner to accept them.
3. Create **`PRIVATE_EVIDENCE/inventory_preflight.json`**: hash all thirteen files completely, including 1 MiB chunk hashes; require nonempty regular files confined to the selected root; compare device/inode/size/mtime/ctime before/after each read and at scan end. This is a private exploratory inventory, **not a manifest accepted by the current validator**. Retain quarantine. Static bounded PE/header/import observations are in **`PRIVATE_EVIDENCE/executable_headers.json`**; they do not identify a supported vendor build.
4. Create **`PRIVATE_EVIDENCE/format_probe.json`** with a bounded read-only Python inspection mirroring `source/data/src/lib.rs:62-185`, `source/data/src/original_payload.rs:32-189`, `source/data/src/bar.rs:22-79` and `source/data/src/replacement_payload.rs:44-143`. Read transform constants from the owned repository implementation; do not write a new test or leave a scratch script. Check all HED ranges and indexed headers. Sample at most two originals per observed storage class per package and two replacements per storage sample, plus finite named selections. Apply 32 MiB payload and 128 MiB cumulative allocation limits; compressed outputs must terminate with exactly declared length/checksum. These are observations from a mirrored format probe, **not execution of Rust data readers**.
5. Correct the guessed `map/us/tt03.map` name using the repository lookup `map/<area>.map` (`source/main/src/area.rs:315`). Create **`PRIVATE_EVIDENCE/named_map_probe.json`** for the found/decoded `map/tt03.map`; keep its identity/selection/output hash private. The initial missing guessed name is not a missing vendor asset.
6. Compile the unchanged zero-dependency `source/game/save/src/lib.rs` with `rustc --edition=2024 --crate-name save --crate-type rlib -F unsafe_code`, then call `save::cipher::Sbox::from_executable` through a minimal temporary adapter. Read the selected executable, not saves, and emit only the result. Record it in **`PRIVATE_EVIDENCE/save_table_reader.txt`**. Remove the temporary adapter/build artifacts. This is a native subcrate/reader probe, not a Cargo workspace build or Linux application smoke.
7. Create **`PRIVATE_EVIDENCE/inventory_postcheck.json`** by fully rehashing all thirteen files and every chunk; require all hashes and metadata to match the preflight observation. No game launch or save access occurred. **`PRIVATE_EVIDENCE/probe_receipt.json`** binds E01, the source observation and evidence artifact identities.
8. Attempt `cargo check -p validation --locked --offline`; it exits 101 before compilation because the existing pinned `skate-data` Git dependency cannot be checked out offline. Do not remove workspace members or alter dependencies/lockfiles as an inspection shortcut. Record native Linux host work separately from source-format evidence.

## Results and checks

| Check | Result |
|---|---|
| Selected KH2FM file set, nonempty/read-only/stable | PASS, thirteen files; no other collection payload inspected |
| Full pre/post file and 1 MiB chunk identity comparison | PASS for all thirteen files; identity is not vendor authenticity |
| Six HEDs and indexed asset headers | PASS for 46,076 records using the mirrored structural probe |
| Per-package record counts | 1,116 / 8,995 / 8,995 / 8,995 / 8,995 / 8,980 |
| Indexed range anomalies | None observed: negative/out-of-bounds ranges, duplicate digests within an index, alignment, overlaps and header/original-length disagreement |
| Observed original storage | 46,020 transformed/zlib; 9 transformed/uncompressed; 47 plain |
| Bounded original samples | PASS, 29 initial plus corrected map = 30; all observed storage classes represented |
| Bounded HD replacement samples | PASS, 8 decoded; one DDS signature observed, not full image/render qualification |
| BAR structure in finite decoded originals | PASS for 23 BAR containers; other samples are not assumed to be BAR |
| Existing native Rust save-table reader against E01 | FAIL: `Err(TableNotAPermutation): the cipher table is not a byte permutation` |
| Actual Epic save algorithm/table/account binding | NOT RUN; no save file accessed or key inferred |
| Cargo validation build | BLOCKED before compilation at existing offline Git dependency |
| Existing Rust data CLI probes | NOT RUN: unavailable binaries and blocked workspace resolution; independent format probe used instead |
| Linux application graphics/audio/input/save/load | NOT RUN; standalone `save` compilation does not establish it |
| Steam/Epic behavior, profile/cache isolation, unknown-build rejection | NOT RUN; no gameplay source profile admitted |
| Workspace fmt/Clippy/tests/build/frame smoke | NOT RUN for this documentation/inspection task; no runtime/test changes, Cargo resolution blocker retained |
| Documentation links/privacy/diff checks | PASS for all four assessment/task documents; no private source hashes, addresses or absolute personal paths found |
| Existing progress report checks | PASS: `python tools/progress.py --check` current; all 9 `tools/test_progress.py` synthetic tests passed |
| Task index regeneration | NOT RUN: `pwsh` unavailable; generated index not hand-edited |
| Coverage/report ledger | Unchanged: finite format probes do not qualify a complete decompiled function or controlled 1:1 behavior unit |

## Uncertainty and verdict

**Read-only inspection completed; Epic support is not implemented or qualified.** The samples support reuse of the current archive format algorithm, with an explicit `en` layout profile. All 46,076 originals/replacements were **not** exhaustively decoded, and model/motion/texture/audio behavior was not checked. Later-package precedence remains an inference; filesystem/profile support must not transplant Steam ordinals or identities. Vendor integrity, exact executable/build differences, correct Epic save-table/account semantics and source-bound runtime behavior remain open.

A path-only change is insufficient: the actual current Rust save-table reader rejects this candidate. Do not enable the current file-menu/save path against E01, treat the exploratory inventory as an accepted validator manifest, bypass selected-source checks or relax quarantine. The separate native Linux audio/window/font/path/controller/packaging work identified in T171 also remains undone. No gate or coverage percentage advances.

Next: if implementation is authorized, first explicitly extend the platform/source contract; restore reproducible dependency resolution; implement native Linux host boundaries and explicit source/language profiles; qualify direct Rust decoding and runtime comparisons; keep native rewrite saves isolated and Epic-save import separately gated. Regenerate the task index with `tools/New-TaskIndex.ps1` when PowerShell is available.
