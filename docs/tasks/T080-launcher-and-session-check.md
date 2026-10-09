# T080 — One launcher and a byte-exact session check

Owner Claude, 2026-10-09. Gate: tooling (supports every play gate). Starts from T079.

User request (2026-10-09): one central exe that opens the different tests; faster start-up, because hashing takes a long time.

## Prerequisites / source

Same quarantined 13-file candidate (observed Steam build 15194255).

## Measurement

The old session admission rehashed all 13 files (15.6 GB) on one core: about 60 s on this machine. The i7-9700F has no SHA extensions, and the SATA SSD reads about 550 MB/s.

## Implementation

**`source_identity` schema 2:**
- Each file also records the SHA-256 of every 1 MiB chunk (`chunks`).
- `chunk_identity` binds all chunk digests.
- The whole-file SHA-256 and `content_identity` are unchanged, so existing configurations still match.
- The scanner hashes files in parallel (`inspect_files`).
- `scan_with(root, false)` skips the chunk digests when only whole-file hashes are compared.

**`package_reader::probe`:**
- **`ProbeSource::verify_session`**, for schema 2:
  - rehashes the executable and every `.hed` whole;
  - requires each package's size and modification time to match;
  - admits the session.
  - Schema 1 manifests fall back to the full rescan.
- **`SourceReader`:** packages are read through it. In a session every byte served comes from a 1 MiB chunk that was just read whole and matched its recorded digest; a mismatch fails the read. A small cache keeps verified chunks.
  - Every package byte a session uses is therefore checked against the full-scan record during that session. Bytes the session never reads are not hashed.
- **`open_index`** refuses until a bound session is admitted.
- `verify_full_source` (research probes) compares whole-file hashes only, in parallel.

**`windows_shell`:**
- The `title` and `test_room` binaries became modules of one binary, `kh2fm`.
- `kh2fm` with no arguments shows a console menu: title, arena room, talk room, each with its private launch file. `kh2fm title|room|talk [args]` runs one directly.
- The title now calls the session check. Before, it bound a session without one.

**Private (outside Git):**
- `%LOCALAPPDATA%/KH2FinalRewrite/source-manifest-02.json`: a one-time full scan in 80 s. Its content identity equals manifest 01.
- The three launch files now point at it.

**README** updated.

## Evidence (private, outside Git)

**Timings (debug build, warm disk):**

| Program | Before | Through `kh2fm` |
|---|---|---|
| Arena room | about 70 s | 7.2 s |
| Talk room | — | 6.0 s |
| Title | — | 3.9 s |

**`autoplay-68` (arena-06, through `kh2fm room`):** the full combat, camp and Items walk-through passes, with no uniform captures.

## Checks

PASS fmt; PASS clippy; PASS tests (new: a session reads verified chunks, refuses before admission, rejects same-size/same-time altered package bytes, and rejects a changed executable); PASS build; PASS smoke; PASS autoplay-68; PASS the menu's input handling (invalid entry, then quit).

NOT RUN: the release build of `kh2fm`.

## Uncertainty

- A session does not hash package bytes it never reads. Those bytes cannot affect it, but a session no longer proves that the whole installation is intact. A full rescan (`verify_full_source`, or a new manifest) still does.
- A chunk is checked when it is read, so corruption shows as a load error at that point rather than at start-up.

## Verdict / next

PASS for the request: one launcher, and start-up about 10× faster with every used byte still verified. Next: the camp Abilities screen (`camp-06/abilities-spec.md`).
