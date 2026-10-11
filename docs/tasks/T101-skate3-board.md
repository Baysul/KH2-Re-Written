# T101 — Skate 3 skateboard on J (disc admission)

Owner Claude, 2026-10-11. Gate: none (owner-requested side feature outside the KH2FM roadmap). Prerequisites: the test room (T088 and later), the launcher.

## Request and scope

The owner asked for J to give Sora a skateboard driven by the Skate 3 Rust engine (SK8-ENGINE/skate-3-rust-engine), with the user's own Skate 3 Xbox 360 ISO required. This is an explicit owner exception to AGENTS.md's "KH2 Final Mix only" rule. No Skate 3 data is stored in Git; the disc is read in place at runtime.

## Reference audit

Clone outside the checkout: `..\skate-3-rust-engine`, commit `2e16697` (shallow).

- License: `GPL-3.0-only` (README "License", `LICENSE`). This workspace is Apache-2.0. Nothing from that repo is copied here. A future dependency on `skate-core` would make any distributed `kh2fm.exe` a GPL-3.0 combined work; the Apache sources stay Apache.
- `skate-core` (~93k lines) is engine-independent (no Bevy, no I/O) but is not callable as "step the skater": `skate-game/src/physics/frame.rs` orchestrates it, and it needs data their Python setup derives from the disc: attrib collections (`skater-collections.json`), animation banks (`*.abin`), physics skeletons, action/motion state graphs and gesture patterns (`*.pat`). The physics runs on the stock Skate 3 skeleton, so Sora would copy the rider's pose.
- `skate-game` pins Bevy 0.18.1 with patched `bevy_pbr`/`bevy_core_pipeline`; this workspace is on 0.19.1, so their game crate cannot be linked.
- Disc admission in their setup (`tools/asset_pipeline/install.py:446-450`) requires `default.xex`, `data/big/{miscload,miscboot,db}.big` and `data/content/createacharacter.big`.

## Implementation

- `skate_source` (new crate): XDVDFS reader working in place on an ISO (partition at 0, 0x2080000, 0xFD90000 or 0x18300000; bounded directory walk), or an extracted folder / `default.xex`. `admit` checks the files above and the XEX2 magic. Tests use generated synthetic images.
- Launcher: settings gain `skate3_iso`; the tests screen has a "Skate 3 ISO" row with a browser for `.iso` / `default.xex`, and dropping an `.iso` on the tests screen sets it. A refused disc shows the reason.
- Test room: J admits the remembered disc. Refused or unset, a notice shows for 300 frames in the system font (lower case and marks encoded after OpenKH's InternationalSystemDecode table). Admitted, it says riding is not built yet.

## Checks

- `cargo fmt --all --check`: PASS. `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS. `cargo test --workspace --locked`: PASS (skate_source 4 tests; launcher settings round-trip covers the new field and old files).
- Visual: arena-07 unattended run with a temporary forced J press (not committed), capture showed the notice above the play area. PASS for the unset case.
- Admitted-disc path against a real Skate 3 disc: NOT RUN (no Skate 3 ISO found on this PC; owner asked for its location).

## Uncertainty

Admission is a layout check; there is no reference digest for a retail Skate 3 disc, so a modified disc with the same files would pass.

## Verdict

PROGRESS: the J key refuses correctly without a disc. Riding is BLOCKED on the owner's ISO.

## Next

With the ISO: read the `.big` archives (RefPack) and the attrib/animation/graph/pattern data in Rust, add `skate-core` as a pinned git dependency, port the per-tick orchestration to Bevy 0.19, and retarget the rider's pose onto Sora with the board mesh from `createacharacter.big`.
