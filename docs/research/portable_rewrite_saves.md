# Portable rewrite saves: constraints and candidate boundaries

Research child: **Establish portable rewrite-save constraints**. Owner: delegated
leaf researcher for project_owner. Baseline: rewrite commit
`554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f`, branch
`research/portable_rewrite_saves`. Prerequisites: fidelity contract, milestones,
unknowns, folder guide, T170, live compatibility map/child/glossary and T172.
This is planning evidence, not an ADR, implementation, source admission or P23
acceptance. Recommendations below require later human decisions.

## 1. What actually persists today

**PASS — owned-code audit; full gameplay persistence is not implemented.**
`save::state::Persistent` describes location, play frames, munny, difficulty,
Roxas selection, level/EXP, weapon, 80 ability slots, four shortcuts, config,
area programs, story flags, arif selection and world records
(`source/game/save/src/state.rs:37-81`). Its read/write adapter edits a retained
vendor-shaped image, rather than replacing unknown fields
(`source/game/save/src/state.rs:83-178`). `SaveData` retains the whole image;
validation checks length, checksum, signature and vendor format version 58,
with explicit little-endian accessors
(`source/game/save/src/data.rs:65-160`). That version is **not** a rewrite
schema or evidence that two sources assign identical meaning to IDs.

A repository-wide symbol search found no production caller of
`Persistent::read`/`write` outside its module. The actual host backend's
`capture()` returns `SaveData::blank()`
(`source/main/src/save_files.rs:85-105`); title loading receives data but discards
it, remembers only the slot and announces a handoff whose next screen is not
built (`source/main/src/title.rs:530-553`, `593-620`). Do not promise that
existing file-menu writes contain current battle or campaign progress.

Authoritative live state is elsewhere: `Progress` owns flags, programs,
signed arif selections, world records, source scripts and destination blocking
(`source/game/area/src/progress.rs:19-66`). Its flag allocation exceeds the
saved subset, and signed arif values cannot simply be cast to the saved bytes
without a defined mapping. `AreaSource::open` constructs it from selected-source
scripts and configured story ranges, then unblocks destinations
(`source/main/src/area.rs:145-185`). Combat separately owns level, EXP, munny,
drops, learned abilities and bonus bookkeeping
(`source/main/src/test_room.rs:2190-2224`). Consequently, neither serializing
`Progress` wholesale nor serializing only `Persistent` establishes a complete
current-state snapshot. Scripts, transient objects and diagnostics are not
thereby new save requirements; the missing capture/apply contract must identify
which existing state survives a supported save boundary.

## 2. Account, executable, menus and location coupling

`Store` derives a key from the decimal Steam account identifier; header masking
also needs a substitution table read from a fixed executable location
(`source/game/save/src/cipher.rs:37-98`). `Store::load` treats only a missing
file as empty and rejects a mismatching key
(`source/game/save/src/store.rs:54-88`). This is legacy container plumbing,
not game-data compatibility or authenticated identity. The container can recover
a mask heuristically from repeated header blocks; the unattended-file backend
uses that path without an account (`source/game/save/src/container.rs:156-178`;
`source/main/src/save_files.rs:35-40`). Neither heuristic recovery nor a valid
checksum qualifies content semantics.

Interactive `FileBackend::open` always reads the selected executable first,
even for an existing rewrite file. It discovers a Windows-style vendor folder,
selects the **lowest numeric account folder containing the filename**, or zero,
and opens one shared rewrite `saves/KHIIFM_WW.png`. If absent, it attempts to seed
from that account's vendor container; any vendor-load error becomes an empty
container. Seeding is initially in memory, not an immediate disk write
(`source/main/src/save_files.rs:43-69`;
`source/game/save/src/store.rs:103-137`). Changed discovery/account/table can
therefore strand an existing rewrite file; an empty display must not be treated
as proof that no progress exists. Existing own-file errors propagate instead
of triggering vendor reseeding.

All current save-menu host wiring is in the title executable path. Its CLI takes
source root, manifest and title config; unattended `--autoplay`,
`--autoplay-files` and `--autoplay-savepoint` use generated memory containers or
`KH2RW_TEST_SAVE`, never the writable interactive backend
(`source/main/src/title.rs:754-811`, `845-853`). The backend is opened before
the optional menu-art error handler, so its executable/table failure is not
merely a hidden menu. T172's unchanged reader rejected E01 with
`Err(TableNotAPermutation): the cipher table is not a byte permutation`
(`docs/tasks/T172_epic_source_probe.md`, Results). Epic vendor import remains
deferred; relocating folders cannot solve this coupling.

The menu offers 99 slots, summary/corruption display, overwrite confirmation,
load and delete; its backend boundary returns `Option<SaveData>` and booleans,
with a separate capture operation
(`source/game/save/src/slots.rs:12-136`;
`source/game/save/src/menu.rs:100-128`, `585-657`). Save/delete mutate the
in-memory container before persistence succeeds
(`source/main/src/save_files.rs:94-100`): failed I/O can leave memory diverging
from disk. Preserve confirmation/navigation behavior, but distinguish
unsupported schema/data from corruption and write failure; acknowledge success
only after the chosen commit boundary.

Do not lose other existing persistence while moving saves:

- `data_dir()` prefers an existing executable-relative development directory,
  otherwise Windows local application data, finally a relative directory
  (`source/main/src/launcher/install.rs:24-39`). Moving the executable can change
  discovery. Vendor discovery guesses Documents/OneDrive roots, not a portable
  account service (`source/game/save/src/store.rs:103-119`).
- Launcher settings include local source/manifest paths, not portable progress
  (`source/main/src/launcher/install.rs:54-79`). Debug **Save Start** persists
  per-world area/entrance choices in `start-area.json`, separately from slots
  (`source/main/src/test_room/debug_menu.rs:703-716`;
  `source/main/src/launcher/install.rs:446-487`).
- Host graphics/audio/language settings live beside the room config in
  `kh2fm-settings.json`; unattended runs suppress that file
  (`source/main/src/test_room.rs:10828-10838`). Their direct write and
  unreadable-file default behavior are distinct from progress
  (`source/visual/src/settings.rs:29-51`, `94-106`). Preserve preferences without
  making monitor modes, paths or source-language availability save identity.

## 3. Genuine alternatives, not accepted decisions

**Candidate A: versioned envelope around the retained legacy image.** Smallest
preservation risk for prior rewrite files and unknown fields, but it retains
vendor layout debt and needs an explicit legacy decoder/profile. Do not require
vendor account discovery or the selected source's cipher table for ordinary
new-save access.

**Candidate B: canonical modeled-state payload.** Separates persistence from
vendor cipher/layout and OS, but is unsafe as a lossless migration today:
`Persistent` is incomplete and not connected to capture/apply. Keep legacy
originals and opaque retained fields separately rather than silently discarding
them. A hybrid envelope with a canonical portion plus preserved legacy material
is possible, at the cost of specifying which representation is authoritative.
No serializer/library or binary layout is selected here.

For either candidate, separate envelope version, gameplay schema and a positive
**game-data compatibility identifier**. Initially, a conservative qualified
profile allowlist can reject everything else; later, reviewed equivalence groups
may admit multiple builds after checking saved world/program/flag/item/ability
semantics and implemented rules. Exact source identity alone is overly strict
for portable equivalent data; storefront/account names are dangerously weak.
Unknown/future versions, malformed lengths and unestablished compatibility must
reject before live-state mutation or overwrite. Revalidate the selected source
every gameplay session independently. Windows/Linux byte portability is not
Steam/Epic semantic equivalence, and neither entails vendor-save import.

Migration should be explicit, non-destructive and recoverable: enumerate legacy
rewrite locations without automatic account selection, retain originals,
validate using the appropriate legacy interpretation, write a separate result,
and verify it before offering adoption. Preserve all slots, metadata and unknown
content, not only modeled fields. Failed migration must not reseed or overwrite.
No user saves were inspected, so no actual migration success is claimed.

## 4. Failure safety and evidence limits

Current `Store::save` writes a fixed sibling `.png.new` then renames, without
explicit synchronization, backup, concurrency control or restart recovery
(`source/game/save/src/store.rs:91-100`). This reduces partial-target exposure;
it does not establish crash durability. Compare:

- Single replacement: bounded complexity; needs exclusive temporary creation,
  serialized writers, checked writes/synchronization and platform-specific
  replacement/recovery handling.
- Retained generations plus validated selection: stronger recovery options and
  preservation, but requires explicit generation ordering and recovery policy;
  timestamps alone are not a safe authority.

Primary sources: [POSIX.1-2024 rename](https://pubs.opengroup.org/onlinepubs/9799919799/functions/rename.html)
specifies atomic name replacement, distinct from
[fsync](https://pubs.opengroup.org/onlinepubs/9799919799/functions/fsync.html)
storage synchronization (including directory-entry discussion).
Microsoft's [ReplaceFileW](https://github.com/MicrosoftDocs/sdk-api/blob/d43984d7c322f7d7d35704d6d1ad36dc3539703b/sdk-api-src/content/winbase/nf-winbase-replacefilew.md)
requires same-volume files, offers backup, documents partial failure states and
says its write-through flag is unsupported;
[FlushFileBuffers](https://github.com/MicrosoftDocs/sdk-api/blob/d43984d7c322f7d7d35704d6d1ad36dc3539703b/sdk-api-src/content/fileapi/nf-fileapi-flushfilebuffers.md)
is a separate operation. Neither documents unconditional power-loss immunity.
These are candidate platform primitives, not proof of the pinned Rust 1.99.0
implementation or tested host filesystem behavior.

[XDG Base Directory 0.8](https://specifications.freedesktop.org/basedir-spec/0.8/)
distinguishes portable user data from less portable state/config/cache; a
Linux user-data save location is a candidate, with explicit legacy discovery,
not silent relocation. No directory-library API is recommended: Context7 skill
was read, but tool discovery returned no Context7 tools (**BLOCKED** lookup).

**PASS:** branch/common-directory/base verification, bounded owned-code audit,
primary-document retrieval. **NOT RUN:** tests/build/runtime, cross-platform
roundtrip, migration, crash/concurrency probes, source equivalence or vendor
import. No source/save access, application launch or test modification occurred.
Next: human selection of boundary/recovery policy, then a separately authorized
capture/apply and compatibility specification with approved synthetic expected
values. No gate or coverage status changes.
