# T105 — Variable-height jump and the Square button's combo abilities

Owner Claude, 2026-10-11. Gates P18/P21 (test-room scope). Starts from `main` at `43685b3`. User request: review the third-party KH2 Combat Kit (`steferro78-HIV/kh2-combat-kit`, MIT) for integration, then, on the owner's "both", port the two gaps the review found.

## Review of the combat kit (no code taken)

The kit is a clean-room library written from a behaviour description and recordings of the game. Its combo gate and attack chooser agree with our exe ports (`ptya.rs`). Its jump is a 15-point table that samples the exe's closed form we already run (`physics::Arc`); it deliberately skips the every-second-frame physics step, takes Combo Plus / Negative Combo / Finishing Plus from the wiki, and fits its lunge speed. It was not integrated. Its recordings did show two behaviours our port lacked: a released jump is cut short, and the Square-button moves.

## Prerequisites / source

Same selected source and private manifest as T056–T103. Ghidra 12.1.4 headless, read-only project `ghidra-kh2-a2`. Outputs are private: `research/playable-room-20261008/jump-01/g/` (`d01`–`d08`, `sc01`–`sc08`, `dd01`–`dd04`, `r01`–`r03`, `s01`–`s02`). The Square entry point was already traced in `guard-01/guard-spec.md` §1.1.

## Findings (installed executable)

1. **Jump flags.** FUN_1403d6fc0 (command dispatch) calls the physics mode's slot +0x48 with 1 for a Jump press (request byte +4 bit 1); FUN_140422630 then sets obj+0x120 bit 16. Mode-1 entry FUN_1403cbd30 copies bit 16 to bit 17 and clears 16. Every dispatch, bit 17 with request bit 0 clear becomes bit 18 [O]; bit 0 is read as "Jump held" [I, paired with the press bit and matching the kit's recordings]. Mode-1 exit FUN_1403cb8a0 clears bits 16–18 [O].
2. **The cut.** The mode-1 update FUN_1403cb680: with bit 18 and `6 < t < rise − 5` (`DAT_140623b9c` = 6, `DAT_140623b98` = 5) it plays the fall motion (FUN_1403cb610) and calls the mode's slot +0x20, FUN_140409b00 → FUN_1403cbc30 + re-entry, with `DAT_142a243d8`. That global is set at start-up by FUN_1400259b0 to g × 0.5 × 5 × 5 ≈ 5.102: a rise lasting exactly 5 ticks. Once `rise ≤ t + 5`, bit 17 is cleared [O]. The same routine cuts the rise with height 0 when FUN_1403b89a0 reports a ceiling (bit 22) [O; not ported, the room collision reports no ceilings].
3. **Square.** FUN_1403d6fc0 runs FUN_1403d6d50 (blow recovery) first: request bit 1 with Aerial Recovery, **or bit 3 (Square)** for Retaliating Slash (PTYA 0x28, counters 1/1) [O]. Our port had Retaliating Slash on Attack. Then Square goes to FUN_1403a81f0, which tries FUN_1403a9950 for PTYA 0x25 (Finishing Leap), 0x0c (Upper Slash), 0x26 (Horizontal Slash), then Guard [O, guard-spec]. FUN_1403a9950: the record must pass FUN_140402750; a non-finisher record (+4 bit 2 clear) is refused while FUN_1403d7780(obj, 0) holds; then ANB range 11 or 50 must be active and FUN_1403d6ca0 must count the press; FUN_1403d78d0 starts it [O].

## Implementation

- `physics`: `JUMP_CUT_AFTER`, `JUMP_CUT_HOP`, `Arc::cuttable`.
- `player_sim`: `JumpCut` (bits 16–18), `Input::jump_up`, the release check each tick, and the cut in mode 1; every other launch, fall and landing clears the flags. `try_square` / `SQUARE_ABILITIES` and `execute` (FUN_1403d78d0, shared with the attack command); Square is tried before Attack in attack and cast states; Retaliating Slash moved to Square; Guard now checks its own ability bit.
- `test_room`: `jump_up` from the live pad (logical 0x100 not held; unattended runs hold it); Square reaches the player without the Guard gate; the unattended run's Retaliating Slash press at tick 1720 is now Square.

## Checks

PASS `cargo fmt --all --check`; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (new: release cuts after the arc passes 6, later releases go higher, past rise − 5 nothing is cut, the cut hop lasts 5 ticks, Upper Slash needs the window, a connected swing and its ability, Retaliating Slash ignores Attack). PASS `cargo build --workspace --locked`; PASS `cargo build --release --features skate3 --locked -p windows_shell`. NOT RUN: interactive playtest; comparison against the installed game.

## Uncertainty

- Request bit 0 as "held" is inferred.
- The order of the dispatch (input update) against the mode-1 update within a frame is assumed (dispatch first); `t` only changes on the physics frames, so the cut tick is the same either way.
- The ceiling cut (bit 22) and FUN_1403cb680's first branch (FUN_14040ea20 when `t` passes `rise + 6`) are not ported.
- Square abilities are tried only from attack and cast motions (the only ones with ranges 11/50 in Sora's set).

## Verdict / next

PROGRESS: the jump is variable and the Square abilities follow the exe's gates. Next: an interactive check of short hops and Upper Slash in the room.
