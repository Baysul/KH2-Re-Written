# T182 — The victim's own hit sound

Owner Claude, 2026-10-17, requested by burba in the project chat (combat sounds
missing, issue #100). Gate: development of the test room only; no final gameplay
acceptance is inferred. Prerequisites: T150 (effects and sounds), T180. Build
identity: branch `claude/combat-feel-victim-sound` on PR 102's branch (`main` at
`554d3bf`), package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam
KH2FM build, validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/`
(Ghidra decompiles `g/d5`, `d12`, `d13`, `d24`, `d31`, `d32`; unattended runs
`snd1` and `tour2` in the private `kh2-combat-out/`).

## What the executable does

- Every hit the damage routine applies to an actor (`FUN_140410d60`, unless the
  hit record's flags bit 2 is set) plays a second sound besides the attacker's
  ATKP HitSound: `FUN_1403dadf0(victim, bank * 1000 + (n & 1) + class, position)`.
  `n` is the victim's hit counter byte (`obj+0x26f*4`, incremented after each
  sound); `class` comes from the ATKP reaction through `FUN_1403ce980`
  (reactions 1 and 12 are class 1; 3, 6, 8, 10, 13, 16, 18 class 3; 4, 7, 9, 11,
  15, 17 class 4): classes 1, 3 and 5 add 0, class 4 adds 2, any other plays
  nothing. `bank` is the short at +2 of the resource record behind `obj+0xbb0`
  (`FUN_14039da60`): the sound bank the victim's object was loaded with
  (`FUN_14039df10` stores the bank argument of the object loader `FUN_1403e0660`
  and `FUN_14039e900` loads it).
- The install has sounds 201000 to 201003, the four ids this scheme gives the
  Shadow, and its effect sounds name bank 201 (201016, 201017, 201019). Banks
  tried for Sora, the other enemies and the system (0, 100, 200, 202, 214, 999)
  have no such ids except bank 0 and the Keyblade's 500.

## Rewrite

`test_room/victim_hit.rs` and `EffectRequest::VictimSound`: a counted hit by
Sora on the room Shadow or a battle enemy asks for the sound; the bank is taken
from the victim's own effect sounds (the most common thousand among sounds of
100000 and up), the counter is kept per victim.

## Checks, uncertainty, verdict and next task

PASS unit tests (reaction offsets; the bank from effect sounds; the alternating
ids; a reaction with no sound leaves the counter). PASS the three-actor combat
tour with the installed source (private `tour2`): the Shadow's hits played 201000
and 201003 (resolved, one layer each). Engineering checks are recorded in the pull
request.

[I] The bank. Where the object loader gets its bank argument was not traced; the
rule above reproduces the one enemy whose sounds exist. For the other battle
enemies the inferred bank (200, 214) has no ids 0 to 3 in the install, so they
stay silent, which may be wrong. Sora as a victim is not built (his bank is
unknown; the ids of banks 0 to 3 resolve but that is not evidence). The
position of the sound and the 0x17-class object branch of the routine are not
modelled.

Verdict: PARTIAL. The victim hit sound follows the traced routine for the room
Shadow; the bank source and Sora as victim remain open (issue #100).
Next: trace the bank argument of `FUN_1403e0660`'s callers.
