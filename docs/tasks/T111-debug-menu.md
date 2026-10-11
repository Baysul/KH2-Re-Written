# T111 — In-game debug menu

Owner Claude, 2026-10-11. Gate: development tooling for the test rooms (no gameplay rule changes). Starts from `main` at 315a6e5. User request (issue #39): a clean in-game debug menu to get items and do test actions. Steam build 15194255 through the session-validated source (`source-manifest-02`).

## What it does

F3 (or both stick buttons on a controller) opens a plain box over the test room, Twilight Town and the combined room; the field pauses while it is open. Arrows / D-pad move, Enter / Space / A choose, Esc / Backspace / B go back, F3 closes. Unattended runs never open it. It is not part of the original game and draws with the system font on the HUD canvas, like the place-name banner.

- **Items:** "Fill every item" sets every item the camp's All tab lists, plus the spell items, to its maximum (99; spells to level 3) and refills empty battle slots. Each item row changes its count with Left/Right. Counts go into the battle stock and the camp stock, and spell items set the spell level at once.
- **Restore HP, MP and Drive:** HP and MP to full (an MP charge is cancelled); the Drive gauge refills unless a form is running.
- **Level up:** grants exactly the EXP to the next level through the same level-up code as a kill (stats from the level rows, the learned ability).
- **Drive form:** enters Valor, Wisdom, Limit, Master, Final or Antiform with a full gauge, skipping the roll, the cost and the party requirement; Revert leaves the form.
- **Invincible:** Sora's hits are taken as during his invulnerability (recorded, no damage, no reaction).
- **Defeat / Respawn / Freeze enemy:** the Shadow dies (the normal kill then gives EXP and prizes), comes back at its spawn point, or stops ticking.
- **Warp:** any area of the current world by its place name, through the normal iris transition, at entrance 0.

## Changes

- `test_room/debug_menu.rs`: the menu (input, rows, actions, drawing).
- `player_sim::shadow::Shadow::defeat` and `respawn` (test-room aids).
- `prize_room::grant_exp`: the kill's level-up loop, now shared with the menu.
- `drive_room::start_change` is visible to the menu; the camp, field pad and Esc-to-exit ignore input while the menu is open.

## Checks

PASS `cargo fmt --all --check`; PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`; PASS `cargo test --workspace --locked` (new: `defeat_and_respawn`, scrolling, spell caps, the menu's draws stay inside its box); PASS `cargo build --workspace --locked`; PASS smoke `cargo run -p windows_shell --locked -- --frames 180`. PASS a 60-frame interactive start of the test room (`test-room-arena-07`). Unattended run of the same room (9400 ticks, 151 captures): exits 1 on this branch and on `main` alike, with the same 151 captures; the cause of that exit was not looked into.
PASS a local probe build (not committed) that drove the menu's own actions during an unattended run of the same room: Fill every item (every listed item at 99), Level up (level 2), Defeat enemy (prize orbs dropped) and Valor Form (entered with a full gauge, Revert offered); the captures show the box, highlight bar, values and footer drawn as intended. Evidence: private captures outside the checkout.
NOT RUN: opening the menu by hand with the keyboard or a controller, Warp in Twilight Town, Invincible, Freeze and Respawn in a run (the owner may be using the PC; no keys were sent to it).

## Uncertainty

- Warping uses entrance 0 of the target area; whether every area's entrance 0 is a usable spawn was not checked.
- Forcing a form skips the roll and the party requirement, so Final Form and Antiform can be entered in states the original would not allow.

## Verdict / next

PROGRESS: the menu is built and wired. Next: the owner's interactive check of each action.
