# T087 — Owner's title edits: shorter copyright hold, "Final Re:WRITE" banner

Owner Claude, 2026-10-09. Gate P22 (front-end scope). Starts from T086. User request: make the copyright screen a little shorter, and change the logo's "Final Mix" to "Final Re:WRITE" in the same style. Both are **deliberate, non-original changes** asked for by the project owner.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Data edited in memory after load: `menu/us/title.2ld` SEQD 1 group 2, and HD replacement 7 (`US_title_2ld7.png`, 1024², the logo parts). Banner measurements were taken from the private preview `research/playable-room-20261008/title-01/hd-preview/US_title_2ld7.png`.

## Implementation (`windows_shell::title_edits`)

- **Copyright hold.** In the installed build, the copyright group has these animation frames: [520,539] ×2, [540,779], [540,599], [720,779], [780,850]. `shorten_copyright` cuts 60 frames (1 s) from the hold:
  - the block's end moves 779 → 719;
  - the fade-out moves to 660–719;
  - the bare-backdrop tail moves to 720–790.

  Scene 8 is now 791 frames long. The edit applies only when the frames match exactly; otherwise it leaves them alone.
- **Banner.** `final_rewrite_banner` rebuilds rows 0–55, columns 64–559 of image 7 when it is 1024². The Disney and Square Enix logos to the right are not touched.
  - The old text (between the ornaments at columns 160–206 and 419–466) is cleared.
  - The ornaments move outward by 51 columns.
  - The end lines (70–159, 467–554) are squeezed by the same amount (nearest column), so their tapers stay.
  - The new text is drawn centred on column 313 from row 12.
- **Text asset.** `crates/windows_shell/assets/final-rewrite-text.png` (314×32).
  - It is drawn by `tools/Make-BannerText.ps1` from the Windows Bahnschrift Bold font, at the banner's cap height (26 px).
  - It uses the gold gradient measured down the "F" stem (87,79,56 at the top to 211,206,168 near the bottom), with a 2 px dark edge (30,28,21) and a soft black halo.
  - It contains no game pixels; the ornaments and lines come from the install at runtime. It is allowlisted by name in `.gitignore`.
- `kh2fm title` applies both edits after loading title.2ld and records `copyright_cut` and `banner` in the log's `loaded` line. It prints a warning if either edit was not applied.

## Evidence (private, outside Git)

- `title-run-19` (+ `.jsonl`): an unattended run.
  - The log has `"banner":true,"copyright_cut":true`.
  - Capture 03 shows the idle title with "Final Re:WRITE" between the moved ornaments.
- `title-run-20`: a local, uncommitted build that keeps both splashes in the autoplay plan.
  - Studio splash from tick 645.
  - The copyright block fades out at capture 12 (tick 1080).
  - The intro sketch shows at capture 15 (tick 1200), consistent with the 791-frame scene 8.

## Checks

- PASS `cargo fmt --all --check`.
- PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- PASS `cargo test --workspace --locked`; 5 new tests in `title_edits`.
- PASS `cargo build --workspace --locked`.
- PASS 180-frame smoke.

NOT RUN: an owner review of the banner's look.

## Uncertainty

- The banner font is the closest available system face, not the original's typeface. The original is not identified [U].
- The squeezed lines use nearest-column sampling [I: no visible step in capture 03].

## Verdict / next

PROGRESS: both edits are in, on top of T086. Next: the owner's review of the banner.
