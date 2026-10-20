# Epic reader boundaries and selected-source probes

## Scope and verdict

Static repository research following T171, followed by Main's bounded read-only observations of a user-selected Epic candidate in section 5 and [T172](../tasks/T172_epic_source_probe.md). The static researcher did not inspect installed files or run builds; those later observations are explicitly distinguished below. This report is **not runtime qualification**, supported-build admission, or evidence that Epic matches Steam.

The existing contract still requires selected-source validation and separates decoding from behavior and presentation (`docs/fidelity-contract.md:5-32`, `docs/fidelity-contract.md:36-42`). T171 records the earlier absent-Epic-evidence and dependency-blocked Linux assessment; it remains historical rather than being retroactively changed (`docs/tasks/T171_linux_epic_feasibility.md:7-39`). No runtime, configuration, tests or coverage ledger were changed.

## 1. Candidate layout is fixed; storefront qualification is absent

- `validation::required_files()` requires the KH2 executable plus six numbered `Image/dt/kh2_*.hed` / `.pkg` pairs: thirteen files. It does not discover alternate language folders or package families (`validation/src/lib.rs:228-239`). On Linux, exact path spelling therefore matters; a different Epic layout is a scanner boundary, not proof of corrupt files.
- The scanner hashes the selected files, checks that links remain inside the selected root, and rejects concurrent size/time changes (`validation/src/lib.rs:86-136`). It does not launch the PE or establish that its contents implement a supported game build.
- Steam ACF metadata is optional. The scanner checks a particular ancestor location, accepts matching app/directory clues when available, and otherwise leaves observed Steam fields empty. Nevertheless its product label and diagnostics remain Steam-specific. Every returned manifest is `quarantined_unqualified`, with an explicit unknown for the absence of an independently qualified supported-build profile (`validation/src/lib.rs:263-329`). Thus a same-layout Epic candidate can potentially be inventoried without Steam metadata; that output must not be described as Steam identification or Epic support.
- `ProbeSource::bind` enforces canonical root, schema/content/chunk identities, the exact required candidate file set and the quarantine string. It does **not** select an Epic/Steam executable profile (`source/data/src/probe_source.rs:142-187`). `verify_full_source` compares current file hashes, sizes and timestamps to the bound manifest; this proves consistency with that observation, not vendor authenticity or gameplay compatibility (`source/data/src/probe_source.rs:208-229`).
- Schema 2 session checking rehashes the executable and indexes, checks package metadata, and verifies chunks as package bytes are served. Schema 1 falls back to a full rescan. This is a source-consistency mechanism, not a compatibility registry (`source/data/src/probe_source.rs:231-285`, `source/data/src/probe_source.rs:78-100`).

**Suggested terminology:** “selected Epic candidate inventoried; quarantined,” “finite package sample decoded,” or “table structurally readable.” Avoid “validated Epic build,” “qualified gameplay source,” and “Linux support confirmed.” No task/build/probe result alone passes P02/P06 or runtime gates.

## 2. HED, storage and decryption assumptions

These are implemented format assumptions to compare with Epic, not observations of Epic files:

| Boundary | Implemented behavior and primary source |
|---|---|
| HED | Nonempty, at most 32 MiB, complete 32-byte records. Each record contains a 16-byte name digest, little-endian signed 64-bit package offset, signed 32-bit stored length and original length. Negative fields and ranges beyond the paired package fail. Duplicate digests, non-16-byte alignment and overlaps are counted, not automatically rejected (`source/data/src/lib.rs:62-80`, `source/data/src/lib.rs:123-185`). |
| Asset header | Four little-endian signed 32-bit fields: decompressed length, replacement count, storage tag and creation date. Negative size/count and tags below -2 fail. The original follows a 16-byte header and `count * 48` bytes of replacement descriptors; declared table plus stored original must fit the HED range (`source/data/src/lib.rs:83-116`). |
| Original storage | Tag -2: plain, uncompressed. Tag -1: transformed, uncompressed. Nonnegative tag: stored compressed length, transformed then zlib-decoded. Selection is by each header's tag, **not storefront, filename or executable identity** (`source/data/src/original_payload.rs:92-138`, `source/data/src/original_payload.rs:150-189`). |
| Transform | One compiled format transform, adapted from the pinned OpenKH `EgsEncryption.cs` source identified in the module attribution. Seed is the container's original 16-byte header; at most the first 256 payload bytes are transformed. There is no alternate Epic/Steam key/profile switch. The upstream filename is provenance, not compatibility evidence (`source/data/src/original_payload.rs:1-4`, `source/data/src/original_payload.rs:32-65`, `source/data/src/original_payload.rs:105-127`). |
| Limits | Original reads default to 32 MiB; a separate API accepts an explicit larger cap. Compressed output must terminate with exactly the declared output length and a valid zlib stream/checksum (`source/data/src/original_payload.rs:9`, `source/data/src/original_payload.rs:74-102`, `source/data/src/original_payload.rs:157-189`). A cap failure need not mean an incompatible format. |
| HD replacements | Located sequentially after the original, aligned to 16 bytes; declared descriptor offsets remain opaque rather than driving file seeks. Descriptors are 48 bytes with a 32-byte name field and offset/size/tag fields. Decode uses the original header seed and shared tag logic. Plain replacement padding is retained. Default sampling reads at most two replacements; explicit all-replacement reading caps count at 64, individual stored/output size at 32 MiB and allocation at 128 MiB (`source/data/src/replacement_payload.rs:1-21`, `source/data/src/replacement_payload.rs:28-143`). |

Package/name assumptions are independently important: `Library` opens six fixed indexes; lookup hashes the exact UTF-8 name bytes using MD5. It finds the first matching ordinal in each index, then chooses the last matching package. The later-package precedence comment explicitly labels that interpretation an inference (`source/data/src/path_lookup.rs:9-17`, `source/data/src/path_lookup.rs:89-124`). Do not carry Steam ordinals or selection digests into Epic. Generate selections from the new private inventory; retain duplicate candidates rather than assuming identical content or patch ordering.

## 3. Executable-derived table boundary

The concrete installed-PE table reader located in the KH2 save path is `save::cipher::Sbox::from_executable(exe: &[u8]) -> Result<Self, CipherError>` (`source/game/save/src/cipher.rs:42-79`). Its assumptions differ sharply from generic package reading:

- `SBOX_ADDRESS` and `IMAGE_BASE` are fixed compiled constants. The reader subtracts them to form one RVA; it does not read the PE's preferred image base or resolve a table from a build-specific profile. Numeric game addresses are intentionally omitted here (`source/game/save/src/cipher.rs:12-14`, `source/game/save/src/cipher.rs:57-76`).
- It checks MZ/PE signatures, reads section count and optional-header length, locates that RVA within the minimum of virtual/raw section sizes, and reads 256 bytes. It then requires a permutation of all byte values. It does not check a supported executable digest, machine type or PE32+ magic; even its “64-bit Windows executable” diagnostic is stronger than those actual checks (`source/game/save/src/cipher.rs:24-29`, `source/game/save/src/cipher.rs:42-93`).
- A readable permutation at the fixed location is **not** proof that Epic uses that table, save algorithm or account identity. A different layout can fail even when packages decode correctly.
- `FileBackend::open` reads the selected executable and immediately invokes that reader, then discovers a Steam account and may read its original save as a starting point. Consequently launching the file menu is not a minimal read-only package probe and should not be used for this investigation (`source/main/src/save_files.rs:43-70`). Account-derived key construction is explicitly Steam-specific (`source/game/save/src/cipher.rs:96-100`).

The package decoder itself does not need this save table or execute the installed PE. The validator merely hashes the executable while inventorying the candidate (`validation/src/lib.rs:96-127`). Also distinguish compiled gameplay interpretations from installed-table extraction: matching package structures cannot establish that Epic's behavior matches translated Steam routines (for example the explicit baseline comments in `source/game/player/src/actor_physics.rs:1-5` and `source/game/enemy/src/act.rs:1-17`). No general executable-profile qualification follows from the source binder.

## 4. Existing CLI probes for Main

Commands below are **suggestions only; none was executed here**. Prefer already available host-native binaries, not the launcher or installed executable. A separate standalone reader build is not a successful workspace build or gameplay qualification. Resolve the root privately and use fresh private outputs outside both installation and Git checkouts. `safe_output` canonicalizes an existing parent and rejects installation/Git ancestors; it does not create that parent (`validation/src/lib.rs:340-359`). Outputs can contain private identities, paths, metadata, hashes or decoded bytes and must not enter public reports.

Here `ROOT`, `MANIFEST`, `INVENTORY`, `REPORT`, `SELECTION` and `PAYLOAD` stand for private arguments, not literal paths.

1. **Candidate identity:** `validation ROOT MANIFEST` — exactly two arguments, new manifest file; scans all thirteen required files and records chunks. Optional Steam metadata is not required. Beware that it may inspect the expected ancestor ACF location if present (`validation/src/main.rs:7-25`, `validation/src/lib.rs:242-250`, `validation/src/lib.rs:289-329`). Stop on missing layout; do not rename, link, repair or edit the installation to force success.
2. **Structural package inventory:** `data ROOT INVENTORY` — exactly two arguments, new output file. Reads six HEDs and each indexed 16-byte asset header, not decoded payloads. It neither takes a manifest nor performs full source admission; it is independently useful for layout/header evidence but must be paired with private pre/post source identity checks for stronger attribution (`source/data/src/main.rs:7-73`).
3. **Small storage-mode sample:** `original_probe ROOT MANIFEST REPORT` — samples at most two originals per storage-tag category per index, plus at most two replacements per sampled original; cumulative decoded budget 128 MiB. This default mode does **not** call `verify_full_source` (`source/data/src/bin/original_probe.rs:46-54`, `source/data/src/bin/original_probe.rs:166-168`, `source/data/src/bin/original_probe.rs:190-251`). Pair with a fresh full scan and post-check rather than describing binding alone as a complete rescan.
4. **One explicitly chosen original:** `archive_probe ROOT MANIFEST SELECTION REPORT PAYLOAD` — exactly five arguments; new report and payload. Selection JSON uses `schema_version: 1`, `source_content_identity`, and exactly one `selections` entry containing `index`, `ordinal`, `name_hash` (16-byte numeric array). All actual identity values stay private. Selection input capped at 1 MiB. This probe performs a full source rescan before reading, checks ordinal/digest identity and post-read package metadata, and exports the decoded original. Non-BAR payloads can still be retained with no BAR table (`source/data/src/bin/archive_probe.rs:9-58`, `source/data/src/bin/archive_probe.rs:88-115`). Prefer this to launching a viewer when a single bounded sample answers the question.
5. **Targeted named export, only if needed:** `named_dump ROOT MANIFEST PRIVATE_OUT_DIR NAME...` — at least one name; MD5 lookup uses the package precedence above. It does not call `verify_full_source`, and uses ordinary file writes that can overwrite output files. Require a fresh dedicated output directory and explicit private source pre/post checks. Absent names are reported and skipped, so a successful exit does not mean every requested resource was found (`source/data/src/bin/named_dump.rs:7-30`).

`original_probe` also accepts `SELECTION` and an optional existing preview directory. Its explicit selections allow 1–64 source-bound index/ordinal/digest entries, read all declared replacements, and can emit payload prefixes and previews; these outputs are particularly unsuitable for public evidence. Full rescan is conditional on inspection options, not merely on having an explicit selection (`source/data/src/bin/original_probe.rs:11-43`, `source/data/src/bin/original_probe.rs:66-96`, `source/data/src/bin/original_probe.rs:166-180`, `source/data/src/bin/original_probe.rs:239-243`, `source/data/src/bin/original_probe.rs:380-382`).

## Recommended bounded evidence order

Main can first establish exact candidate layout and a private source manifest, then collect structural HED/header inventory, then select finite examples covering observed storage modes and replacement layouts. Keep missing formats, failed decodes and untested categories explicit. Only inspect executable tables separately if needed; package success must not bypass the fixed-RVA/save boundary. Repeat private identity checks after probes. No source-profile addition, save/menu launch or gameplay claim is warranted by these checks alone.

**Static-research checks:** repository-source trace completed; the static researcher did not run installed-file probes. Main's subsequent results follow. No gate or coverage status is advanced.

## 5. Selected Epic candidate observations (T172)

Candidate **E01** is bound to a private full-file/chunk inventory. It remains `quarantined_unqualified`; vendor integrity and an independently supported build identity have not been established. Exact evidence files and reproduction steps are in [T172](../tasks/T172_epic_source_probe.md). Only KH2FM's executable and six KH2 archive pairs were read; no other collection game's payload, vendor save or game process was opened.

### Layout and finite format compatibility

- All thirteen selected files were nonempty and stable. Full SHA-256 and 1 MiB chunk hashes before and after the probes matched for every file. These establish the observation's identity, not vendor authenticity.
- This candidate uses `Image/en/kh2_*.hed` and `.pkg`; the current scanner/library hardcodes `Image/dt/`. This is a concrete layout difference, not a reason to rename, link or modify the installation. Folder selection needs an explicit source/language profile; neither `en` nor `dt` alone identifies a storefront or supported build.
- All six HEDs and all **46,076** indexed asset headers passed the mirrored structural checks. Per-package counts were 1,116 / 8,995 / 8,995 / 8,995 / 8,995 / 8,980. No negative/out-of-range records, duplicate digests within an index, misaligned offsets, overlapping indexed ranges or original-length disagreements were observed.
- Observed original storage modes: **46,020 transformed/zlib**, **9 transformed/uncompressed**, **47 plain**. A bounded Python inspection mirrored the repository's storage transform and range/zlib/BAR checks, taking its format constants from the owned decoder source. It decoded 29 originals plus a separately corrected map selection (**30 total**) and **8 HD replacements**, without observed decode failures. This was **not execution of the Rust data readers**; their Cargo build remains blocked. The initial sample charged about 13.5 MB against a 128 MiB cumulative allocation budget, with 32 MiB per-payload limits.
- The finite selections included battle/system/object tables, Sora model/motion, a room archive and its map. Twenty-three decoded originals had structurally accepted BAR containers; one decoded HD replacement had a DDS signature. No rendered mesh, full texture decoding, motion behavior, audio playback or scene equivalence follows from those checks.
- An initial guessed name, `map/us/tt03.map`, was absent. The repository actually requests `map/tt03.map` (`source/main/src/area.rs:315`); that corrected name was found and decoded. The failed guess is not evidence of a missing map.

These observations support reuse of the existing **format algorithm for the sampled assets**, not byte identity with Steam, exhaustive asset support or new-build gameplay admission. Package precedence and unseen payloads still need qualification.

### Confirmed executable/save boundary

The unchanged, dependency-free `save` library was compiled natively with `rustc`; a minimal temporary read-only adapter called `save::cipher::Sbox::from_executable` on E01. The actual existing Rust reader returned:

```text
Err(TableNotAPermutation): the cipher table is not a byte permutation
```

The temporary adapter/build files were removed. This independently confirms a concrete rejection by the current fixed-location reader; it does not identify Epic's correct table, save algorithm or account key. **Do not enable the current save/menu path against this candidate or assume a path-only change is sufficient.** No vendor save was inspected, imported or written. Native rewrite persistence and optional Epic-save import should remain separate work.

### Remaining blockers

`cargo check -p validation --locked --offline` still stops before compilation because the workspace's existing pinned `skate-data` Git dependency is unavailable locally. No dependency, workspace member or lockfile was changed to bypass it. Full native Linux application build/window/audio/controller checks, direct Rust data-reader execution, supported-source/profile/cache isolation, Epic behavior comparisons and save compatibility remain **NOT RUN**. The source/runtime contract and coverage ledger remain unchanged.
