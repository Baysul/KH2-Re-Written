# Repository reorganization plan

Historical proposal. The user authorized T129 restructuring on 2026-10-15.
Use [folder-guide.md](folder-guide.md) for current paths and naming rules.
Earlier approval/hold notes below describe the state at the time.

Status: **partly applied; see [Status](#status) at the end.** Items not marked done there were held, and each held item says why. The current layout is described in [ARCHITECTURE.md](../ARCHITECTURE.md).

Checked 2026-10-09 on branch `claude/project-thread-98hhql` (in sync with origin). Uncommitted work from the "1:1 title sequence" thread (`windows_shell/src/bin/kh2fm/title.rs`, `src/lib.rs`, untracked `src/splash.rs`) and the "Playable Sora test room" thread (camp, test room, music) is in progress. Phases avoid those files until those threads push.

## Principles

- Game data, manifests, selections, captures and saves stay outside the checkout (AGENTS.md). `.local/` is ignored and holds only development outputs.
- Keep the paths AGENTS.md names: `docs/fidelity-contract.md`, `docs/milestones.md`, `docs/unknowns.md`, `docs/tasks/`.
- A moved file or folder changes every reference in the same commit. No compatibility re-exports or path aliases (AGENTS.md: change the code rather than add shims).
- Folders group by what a file is for (format, layer, purpose), not by when it was added. Task IDs stay the stable key for history.
- No new crate or folder until code needs it. Index files (READMEs) are cheap and come first.

## Where do I start? (developer map)

This is the table a new developer should be able to use without reading the task history.

| I want to... | Look in |
|---|---|
| Understand the layers and crates | `ARCHITECTURE.md` (root) |
| Set up, build and run checks | `CONTRIBUTING.md` (new, A1) |
| Know the rules (scope, private data, gates) | `AGENTS.md`, `docs/fidelity-contract.md`, `docs/milestones.md` |
| Find a document by purpose | `docs/README.md` (new, A2) |
| Read why a decision was made | `docs/decisions/` |
| See what a unit of work proved | `docs/tasks/` (index: A4) |
| Read how a file format is decoded | `crates/package_reader/src/formats/` (phase 5) |
| Find a verification or extraction script | `tools/README.md` (new, A3), then `tools/` |
| Change the player-facing game | `crates/windows_shell/src/` and `crates/player_sim/` |
| Run a developer preview or inspector | `crates/research_tools/` (phase 3) |
| Check release and third-party licences | `docs/release/` |

## Target folder structure

```
KH2FinalRewrite/
  AGENTS.md  README.md  ARCHITECTURE.md  CONTRIBUTING.md (A1)
  LICENSE  NOTICE  Cargo.toml  Cargo.lock  rust-toolchain.toml
  .gitignore  .gitattributes
  .github/
    workflows/ci.yml
    pull_request_template.md (A5, optional)
  crates/
    source_identity/          unchanged. Layer 1: detection and manifests.
    package_reader/           Layer 2: decoding. Reorganized in phase 5.
      src/lib.rs  src/main.rs
      src/archive/  src/text/  src/tables/  src/image/  src/model/  src/vu/
      src/motion/  src/room/  src/audio/  src/effect/  src/inspection/
      examples/               boundary checks (stays)
      src/bin/                nine developer probes (moves in phase 3 if M7 is approved)
    effect_vm/                unchanged. Layer 4: particle VM.
    player_sim/               unchanged for now. Layer 4: test-room controller.
    audio_out/                unchanged. Layer 5: mixer and Windows output.
    windows_shell/            Layer 5: presentation and the kh2fm launcher.
      src/lib.rs  src/main.rs (smoke app)
      src/render/  src/input/  src/screens/   presentation modules (phase 5)
      src/bin/kh2fm/          player launcher (unchanged)
      assets/                 development-checker.bmp (splash decision: G5)
    research_tools/           NEW (phase 3). Developer preview and inspector binaries.
      src/bin/
    research_probes/          NEW (phase 3, optional). Package probes.
      src/bin/
    game_core/                NEW (phase 4, only when the first module moves). Layer 4 game state.
  docs/
    README.md                 NEW (A2): index by purpose
    fidelity-contract.md  milestones.md  unknowns.md         (AGENTS.md paths; unchanged)
    architecture.md           deleted in phase 2 (replaced by ../ARCHITECTURE.md)
    tasks/                    records T001 to T085 plus T001R; index README (A4)
    decisions/                ADRs (0001-foundation.md)
    research/                 NEW (phase 2): inspection notes, tool notes, ledgers
    release/                  provenance, dependency inventory, licence texts
    scenarios/                scenario evidence format
  tools/
    README.md                 NEW (A3): index by purpose; no file moves
    fixtures/                 authored conformance inputs
    (about 50 entries: Verify-*, Build-*, Invoke-*, Inspect-*, native/managed readers, C++ references)
  .local/                     ignored; development outputs only
    test-room/                launch*.json only (double-click bundles; they point outside)
  %USERPROFILE%/KH2FinalRewrite-private/   private, outside the checkout and OneDrive
    research/                 research outputs
    research/requested-repos/ pinned reference snapshots
    research/tools/openkh/    OpenKH clone
    logs/                     launcher and smoke logs
      evidence-2026-10-03/    smoke and metadata files cited by T001 and T023
    handoff/HANDOFF.md        optional
  (test-room source manifests stay in .local/test-room/: LOCALAPPDATA is virtualized for user launches)
```

## New files (proposed)

| # | File | Purpose | Phase |
|---|---|---|---|
| A1 | `CONTRIBUTING.md` | Setup: Rust 1.99.0 from `rust-toolchain.toml`, Cargo bin directory on PATH, the five required commands, the smoke run, where private files go, how to add a task record, what must never be committed. Written for people; `AGENTS.md` stays the rules for agents and contributors | 2 |
| A2 | `docs/README.md` | Index of every file in `docs/` grouped by purpose (start here, rules, research, tasks, decisions, release, scenarios) | 2 |
| A3 | `tools/README.md` | Index of the scripts by purpose, with the task record that cites each one. Gives navigation without moving files (M9 stays deferred) | 2 |
| A4 | `docs/tasks/README.md` | Index table: ID, title, gate, verdict. Keep it generated by a small script, not hand-edited, so it cannot drift from the records | 2 |
| A5 | `.github/pull_request_template.md` | Sections for Before, After, How, checks run, and evidence location. Optional | 2 |

Only the A-files are new in phase 2. No file is created for an empty subsystem.

## Proposed deletions (need approval)

Each item was checked with `git grep` against tracked files. Items that task records cite as evidence are not deleted; they move to private storage (M10).

| # | Path | Size / state | Why | Check before deleting |
|---|---|---|---|---|
| D1 | `target/` | Cargo build output, git-ignored, **55 GB** | Regenerated by `cargo build`; largest item on disk | None |
| D2 | `.local/smoke-first.*`, `smoke-interactive.*`, `smoke-keyboard.*`, `smoke-final.*` (12 files) | Logs from 2026-10-03 smoke runs | **Cited by T001 and T023 as evidence.** Move to private storage (M10), not delete | Only after those records point at the new location |
| D3 | `.local/cargo-metadata.json` (1.5 MB), `.local/dependency-licenses.json` | Generated 2026-10-03 | Cited by T001 and T023 as evidence. The tracked record is `docs/release/dependency-inventory.csv` | Same as D2; move to private storage (M10) |
| D4 | `gb_splash.png` (repo root, ignored) | Byte-identical to `crates/windows_shell/assets/glurbus-splash.png` (MD5 0f55bcf6...) | Scratch copy. Delete only after the title thread confirms the asset path (G5) | Title thread |
| D5 | `.local/tools/iw4l/` (22 MB) | Clean clone of another game's engine (vladtrc/iw4L). Git tree clean | AGENTS.md: KH2 Final Mix only | Confirm no local notes before deleting |
| D6 | `.local/tools/skate3/` (39 MB) | Clean clone of another game (SK8-ENGINE skate-3-rust-engine). Git tree clean | Same | Same |
| D7 | `docs/architecture.md` (tracked) | Stale: describes three crates and says no decoder exists | Replaced by `ARCHITECTURE.md` | Delete in phase 2, after its content is confirmed in `ARCHITECTURE.md` |
| D8 | `crates/package_reader/examples/texa_graphs.rs` | Tracked; no doc or task record names it | Probably unused | **Review**: confirm no task record names it under another name |
| D9 | `crates/package_reader/examples/bc3_mip_boundaries.rs` | Tracked; referenced only by itself | Possibly superseded by `bc3_boundaries.rs` | **Review** the same way |
| D10 | `tools/Compare-Rgba.ps1`, `tools/Verify-FontCells.ps1`, `tools/Verify-ModelPose.ps1`, `tools/Verify-NativeFunctionTable.ps1`, `tools/Verify-NativeIndirectChain.ps1`, `tools/Verify-NativeRipReads.ps1` | Tracked; the file name is cited by no doc or task record | Possibly abandoned checks | **Review**: confirm none is a gate check before deleting |

Not proposed for deletion: `crates/package_reader/src/bin/motion_sample_probe.rs` (its header says it produced evidence; no doc names it, so check before any decision); `tools/*` that task records cite (for example `Verify-ModelSkeleton.ps1`, cited by seven files); `docs/tasks/*` (evidence records).

## Proposed moves (need approval)

| # | From | To | Phase | Reference updates needed |
|---|---|---|---|---|
| M1 | `.local/test-room/source-manifest-01.json`, `source-manifest-02.json`, `test-room-*.json`, `title-01.json`, `actor-selection-01.json`, `axel-selection-01.json`, `shadow-selection-01.json`, `weapon-selection-01.json`, `map-survey-01/` | `.local/test-room/` (stays; corrected from LOCALAPPDATA, which is virtualized for user launches) | 1 (after the playable room thread pushes; it uses these files now) | Update the `.local/test-room/launch*.json` files (local, untracked) and README launch instructions; the `docs/tasks/T056` pointer if it names the path |
| M2 | `.local/tools/requested-repos/` | `%USERPROFILE%/KH2FinalRewrite-private/research/requested-repos/` | 1 | `docs/tool-prerequisites.md` and `docs/bridge-inspection.md` mention this path |
| M3 | `.local/tools/openkh/` (126 MB) | `%USERPROFILE%/KH2FinalRewrite-private/research/tools/openkh/` | 1 | `docs/tasks/T058-motion-reference-match.md` cites the local OpenKH checkout |
| M4 | `HANDOFF.md` (repo root, ignored) | `%USERPROFILE%/KH2FinalRewrite-private/handoff/HANDOFF.md` | 1, optional | None tracked; a stale note from 2026-10-08 |
| M5 | `docs/*-inspection.md`, `docs/tool-*.md`, `docs/bridge-inspection.md`, `docs/environment.md`, `docs/references.md`, `docs/source-manifest-schema.md`, `docs/supported-builds.md`, `docs/source-and-release-policy.md`, `docs/*.csv` | `docs/research/` | 2 | Every `docs/...` link in the repo, README included. One link sweep, then `git grep` for old paths |
| M6 | `crates/windows_shell/src/bin/{room_scene,model_inspector,image_inspector,motion_eval_compare,motion_ik_probe}.rs` | `crates/research_tools/src/bin/` (new) | 3 (after the title and playable-room threads push) | README and task records (T026, T034, T041, T056, T057, T058, T063) cite `cargo run -p windows_shell --bin ...`; update the citations. `room_scene` is the documented preview |
| M7 | `crates/package_reader/src/bin/*_probe.rs` (nine) | `crates/research_probes/src/bin/` | 3, optional | Many task records cite `cargo run -p package_reader --bin ...`. Recommend leaving them unless the split is needed |
| M8 | `windows_shell` game-state modules: `camp.rs` + `camp/`, `command.rs`, `dialogue.rs`, `newgame.rs`, `gauge.rs`, `fade.rs`, `effects.rs`, state part of `title.rs` | `crates/game_core/` (new) | 4, only when a tested behavior requires the boundary | Bevy imports removed or moved to presentation first; the launcher and test room import these |
| M9 | Flat `tools/` (about 50 entries) into subfolders | Not recommended now. The A3 index gives the navigation instead | Deferred | Many task records cite scripts by name; the link sweep is not worth it yet |
| M10 | `.local/smoke-*.log`, `.local/smoke-*.stdout.log`, `.local/smoke-*.stderr.log` (D2), `.local/cargo-metadata.json`, `.local/dependency-licenses.json` (D3) | `%USERPROFILE%/KH2FinalRewrite-private/logs/evidence-2026-10-03/` | 1 | T001 and T023 record paths: update those path lines, keep the evidence |
| M11 | `crates/package_reader/src/*.rs` (flat, 44 modules) | `src/archive/`, `src/text/`, `src/tables/`, `src/image/`, `src/model/`, `src/vu/`, `src/motion/`, `src/room/`, `src/audio/`, `src/effect/`, `src/inspection/` (table below) | 5 | Callers in `windows_shell`, `effect_vm`, `source_identity` and the probes update their `use` paths in the same commit. Module doc headers stay as they are |
| M12 | `crates/windows_shell/src/` presentation modules: `hud.rs`, `layout2d.rs`, `sprite2d.rs`, `ui2d.rs`, `ui2d.wgsl`, `input.rs`, `pad.rs`, `room.rs`, `title.rs` (presentation part), `splash.rs`, `sound.rs` | `render/` (hud, layout2d, sprite2d, ui2d, ui2d.wgsl), `input/` (input, pad), `screens/` (room, title, splash), `sound.rs` stays at the root | 5 (after M8) | Call sites in `lib.rs` and `bin/kh2fm`; no logic changes |

### package_reader grouping (M11)

Groupings follow each module's own header comment (checked for the modules listed). Modules marked "confirm" need a header read before the move.

| Folder | Modules | Basis |
|---|---|---|
| `archive/` | `bar`, `plain`, `original`, `objentry`, `ptya`, `replacement` | BAR and the containers that hold game files; `replacement` is the bounded payload reader |
| `text/` | `message`, `message_text`, `font`, `font_cell` | Message and font data |
| `tables/` | `battle_tables`, `command_table`, `item_table`, `journal_table`, `preferences` | Fixed-layout tables (`00battle.bin` and similar). `preferences`: confirm |
| `image/` | `hd_image`, `image_structure` | Image containers and pixel structure |
| `model/` | `model_geometry`, `model_inspection`, `model_inspection_textures`, `model_packets`, `model_pose`, `model_skeleton`, `model_texture`, `model_texture_animation`, `model_texture_pixels`, `model_texture_routing`, `sklt` | Model decoding; `sklt` is the weapon-attachment table from the system archive |
| `vu/` | `vif`, `vpu` | Lossless VU and VIF data views (the names are PS2 hardware terms; the folder says what they are) |
| `motion/` | `motion`, `motion_eval`, `motion_trigger`, `sequence` | Motion data. `sequence`: confirm |
| `room/` | `room_collision`, `room_geometry`, `object_collision`, `layout` | Room geometry and collision. `layout` (2LD scenes) can move to `room/` or `scene/`: decide in phase 5 |
| `audio/` | `ogg`, `scd` | Audio containers |
| `effect/` | `effect` | Object effect data (PAX and DPX). Name note below |
| `inspection/` | `inspection`, `probe` | Shared developer helpers used by probes and inspectors |
| root | `lib.rs`, `main.rs` | Public entry points stay |

Name note: `package_reader::effect` decodes effect data; `effect_vm` runs it. The two names are close. Keep both for now, and state the split in `ARCHITECTURE.md` (Decision D-1 below).

### windows_shell target (after M8 and M12)

```
windows_shell/src/
  lib.rs  main.rs
  render/      hud, layout2d, sprite2d, ui2d, ui2d.wgsl
  input/       input, pad
  screens/     room, title (presentation), splash, camp/ (presentation parts)
  sound.rs
  bin/kh2fm/   launcher, test_room, title (presentation)
  assets/      development-checker.bmp
```

## Phases and gates

Each phase is a separate commit. Any phase that touches code passes `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo build --workspace --locked`, and `cargo run -p windows_shell --locked -- --frames 180`.

- **Phase 0 (this change, no moves).** `ARCHITECTURE.md`, this plan, and the `.gitignore` allowlist line for `ARCHITECTURE.md`. Done.
- **Phase 1 (after approval; no tracked code touched).** D1, M10, D5, D6 (after confirming each clone is not needed), M1, M2, M3, M4. Local file moves and deletions only. Before running: no session of the playable test room or title threads reads `.local/test-room/`.
- **Phase 2 (after approval; docs only).** M5, D7, A1 to A5. Verify that no link points at an old path (`git grep -n 'docs/<old-name>'`) and that `docs/tasks/*` still resolves. The indexes (A2 to A4) are the cheapest way to make the repo readable before any code moves.
- **Phase 3 (after the title and playable-room threads push).** M6, M7 (if approved), the D8 and D9 decisions, and G5 for `glurbus-splash.png`. Code moves. Full check list.
- **Phase 4 (only when a tested behavior needs it).** M8. Creates `crates/game_core`. Not scaffolded before then (AGENTS.md).
- **Phase 5 (after phase 4 and after the owning threads finish their branches).** M11 and M12. Pure moves with `use` path updates, no logic changes. One commit per crate. Each commit's checks must pass before the next.

## Decisions

Status: the recommendations below are recorded as the working decisions. Recording them does not approve any move, deletion or new file. Those still need item approval.

- **D-1: `package_reader::effect` vs `effect_vm`.** Keep both names. Document the split in `ARCHITECTURE.md` (recorded).
- **D-2: `tools/` layout.** Keep flat, with the A3 index. No link-sweep grouping now (recorded).
- **D-3: Research binaries.** Move them to a `research_tools` crate in phase 3 (recorded; needs M6 approval).
- **D-4: `glurbus-splash.png`.** Stays git-ignored until the title thread completes a provenance review. No action in any phase before that (recorded).
- **D-5: Package probes (M7).** Leave them in `package_reader/src/bin/` until phase 5 (recorded; M7 not approved).

## Gaps this plan does not fix (listed for decision)

- **G1: Private data in `.local/`.** Covered by M1 and M10.
- **G2: Unrelated game clones.** Covered by D5 and D6.
- **G3: `.local/tools/` is a research workspace.** After M2 and M3 it would hold only what cannot move; consider removing the folder.
- **G4: `.gitignore` allowlist.** Adding `ARCHITECTURE.md` is part of phase 0. Each new allowlist entry needs a provenance review (AGENTS.md).
- **G5: `glurbus-splash.png` is git-ignored** but `splash.rs` loads it, so a clean clone cannot show the splash. Options: allowlist it after a provenance review and an owner decision (D-4), or load it from a private path. The title thread decides. Blocks D4.
- **G6: Presentation and game state share one crate.** Phases 4 and 5 address this; nothing earlier.
- **G7: No index of task records.** A4 fixes this without editing the records.

## Approval needed

Reply in the project thread with the item numbers you approve (for example "D1, M10, M1 to M4, phase 2 as proposed") and decisions D-1 to D-5. Nothing in phases 1 to 5 runs before that reply.

## Status

Applied in one batch on 2026-10-09 (uncommitted at the time of writing).

Done:

- Deleted: D4 (root `gb_splash.png`, byte-identical to the crate asset; the title-thread check in its row was not run separately), D5 and D6 (`.local/tools/iw4l/`, `.local/tools/skate3/`, not KH2 Final Mix; `.local/tools/` removed), D7 (`docs/architecture.md`, replaced by `ARCHITECTURE.md`).
- Moved: D2, D3 and M10 (14 evidence files to `%USERPROFILE%/KH2FinalRewrite-private/logs/evidence-2026-10-03/`), M2 (requested repositories), M3 (OpenKH checkout), M4 (`HANDOFF.md`).
- M5: 13 markdown files and 3 CSV ledgers moved to `docs/research/`. All 126 relative links in tracked markdown resolve. `.gitignore` allows `docs/research/*.csv`.
- A2, A3, A4: `docs/README.md`, `tools/README.md`, and `docs/tasks/README.md` (generated by `tools/New-TaskIndex.ps1`; 7 of 88 records have no verdict line, and the index says so).
- A1, A5 and `ARCHITECTURE.md` were added earlier in this thread.

Kept:

- D1 (`target/`): kept at the user's request.
- D8 (`texa_graphs.rs`) and D9 (`bc3_mip_boundaries.rs`): they write the input files that `tools/Verify-ModelTextureAnimations.ps1` and `tools/Verify-Bc3MipImages.ps1` read. Deleting them would break those verifiers.
- D10 (six `tools/*.ps1` files): no task record cites them. `tools/README.md` lists them with no citation or with only this plan as the citation. Each needs a review of whether it is a gate check before deletion. Not reviewed yet.

Held:

- M1 (test-room launch files): the playable-room thread is still using them in `.local/test-room/`, and that folder stays because LOCALAPPDATA is virtualized for user launches.
- M6, M7, M8, M11, M12 (code moves): they wait for the title, launcher and playable-room threads to push, so they do not conflict with in-flight work.
- M9 (flat `tools/` folder): deferred, as the plan recommends.
