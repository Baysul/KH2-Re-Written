# T061 — Sora's field gauge (HP ring, MP bar, Drive gauge)

Owner Claude, 2026-10-09. Gates P18/P21 (test-room scope). Starts from T060 (`7421daf`). User goal: HUD should look exactly like the original. On 2026-10-09 the user allowed routines from the executable to be translated directly into Rust and committed; game assets stay out of Git and are read from the installed source at runtime.

## Prerequisites / source

Same selected Steam source and private manifest as T056–T060 (quarantined; full rehash at every launch). Ghidra 12.1.4 on the read-only installed executable; private outputs under `%LOCALAPPDATA%/KH2FinalRewrite/research/ghidra-kh2-a1/hud-0{1..5}` and `ghidra-kh2-a2/hud-{sprite,seqplayer,pred,sched}`; written behaviour spec `research/playable-room-20261008/hud-01/hud-spec.md`. Public layouts: OpenKH `Sequence.cs` (SEQD tables) and `00battle.md` (PLRP), pin `7a3b945c538d32c6a285128c98aefba093f52ceb`.

## Findings

1. The field HUD loads `field2d/<lang>/<world>0field.2dd` (format string at 0x1405b18d0). It holds four (IMGD, SEQD) pairs: `fiel`, `gaug`, `mapj`, `miss`; Sora's gauge uses `gaug`. The PC package carries one HD PNG replacement per IMGD, in IMGD order (gauge: 1024×512 for a 512×256 IMGD).
2. GAUGE_PLAYER (vtable 0x1405b2100) builds about 20 sprites and depth masks parented to a root circle (intro group 136, then 137 / low-HP 132), all with an X origin of 85 (the 4:3 layout centred in 16:9). Children sit at the parent's child context.
3. The HP ring is filled by a depth trick: depth-only masks (z 200/600/2000) drawn before depth-tested rings (z 100/500/1000). Below 60 HP, the mask angle is HP × π/40; at 60 and above, a bar grows left by 3 px per HP. Changes restart a red damage-shadow ring/bar (groups 126/127). Low HP is HP ≤ MaxHP/4 with integer division and swaps groups from tables at 0x14071afe0/afec/aff8.
4. MP is 1 px per MP. The Drive fill is a 50×120 mask rotated by 0.23° per point. Level digits come from the table at 0x14071b108, and a MAX badge shows when the level reaches the maximum.
5. Sora's starting HP/MP come from PLRP entry id 0, character 1: HP 20, MP 100.
6. The face portrait comes from the character's own archive: `obj/P_EX100.a.us` holds an IMGD + SEQD named `face` (the gauge reads it through the player entity, FUN_14017e980). Its group follows the gauge state: normal 3, low HP 0, KO 2 (FUN_14017eba0). The coordinates in FUN_14017e980 (0x3500/0x2800 plus image size) are texture-memory placement, not screen position. The face sprite is a root child, so its own sequence positions it. Its HD replacement is the last of nine in that entry (the first eight are 128×64 effect DDS files) and is a 1024×512 BGRA DDS without the pitch flag.

## Implementation

- `package_reader::sequence`: SEQD reader plus the executable's player: set/advance/loop/finish rules, Bezier easing, the active-animation test, child context from the first active IsActive animation, and quads with external scale/rotation/colour modifiers.
- `windows_shell::hud`: source-bound sheet loader (sequence + HD replacement; IMGD replacements are the archive's last ones, in IMGD order), plus a PLRP starting-stat reader.
- `package_reader::hd_image`: BGRA8 DDS admission also accepts a header carrying only the required flags (0x1007) when the stored pitch is tight. A test covers this.
- `windows_shell::gauge`: GAUGE_PLAYER translated: element creation and draw order, ring masks, bars, damage shadow, low-HP state, MP, normal Drive mode and MAX badge. A software canvas reproduces the masks' depth test at HD texel resolution; tested with synthetic textures.
- `test_room`: optional `hud` and `face` locators. The gauge ticks at 60 Hz after the simulation and is shown through a 2D camera as a full-window overlay. Test-room keys: H/G change HP by 2, M/N change MP by 10, B adds 10 Drive. The unattended script changes HP and MP so captures show each state.

## Evidence (private, outside Git)

`research/playable-room-20261008/hud-01/` (selection, report, extracted archive, HD previews, gauge contact sheet), `test-room-tt08-06.json` and `autoplay-25` (gauge), and `test-room-tt08-07.json` and `autoplay-26` (gauge plus face). Face sheet files are in `hud-01/` (`afm-*`, `P_EX100.a.us`, `afm-raw/`).

Inspected captures:
- Full HP 20/20: the green ring runs from 9 to 12 o'clock, with DRIVE, a MAX badge, digit 3 and a full MP bar.
- At 14 HP: green ring plus the red damage-shadow segment.
- At 4 HP: low-HP red circle and short green arc.
- At 50 MP: a half-filled MP bar.
- autoplay-26: Sora's portrait in the circle, switching to the tired face at low HP.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (157 tests); PASS build; PASS 180-frame smoke; PASS autoplay-25; PASS release `test_room` build. NOT RUN: interactive playtest; side-by-side capture against the installed game.

## Uncertainty

Not implemented: MP charge, form/summon/locked drive modes and their effects, hit-shake events, the low-HP alarm sound and party/enemy gauges. The 16:9 mapping (layout space stretched over the window) is inferred from the 85-unit origin. The mask rotation sense is inferred from the mask geometry. Blend modes 1/2 are approximated on a transparent canvas. Drive level 3 of 3 with an empty partial gauge is a test-room choice.

## Verdict / next

PROGRESS: Sora's gauge now comes from the game's own sequence data and the translated gauge logic. Next: the command menu, then the remaining drive modes.
