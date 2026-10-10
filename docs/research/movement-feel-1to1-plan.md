# Movement and combat feel: 1:1 plan

Research and plan only. No code changes come with this document.

| Field | Value |
| --- | --- |
| ID | R-MOVE-01 (a `docs/tasks/` number is assigned when step 1 starts, to avoid colliding with threads that are open now) |
| Gate | P18/P21 test-room scope: Sora locomotion, jump, air, knock-back |
| Owner | Claude, 2026-10-09 |
| Prerequisites | T054–T074 (PRTY/SSTM readers, ANB triggers, combat and guard traces) |
| Build / source | Steam build 15194255, `KINGDOM HEARTS II FINAL MIX.exe` 1.0.0.2, read-only in the private Ghidra projects `ghidra-kh2-a1..a4`; the source stays quarantined and is revalidated at every launch |
| Evidence | `%LOCALAPPDATA%\KH2FinalRewrite\research\playable-room-20261008\movement-01\g\` (decompiles `x01`–`x19`, physics-mode vtables `pm01`, constant dumps `d01`–`d03`, ref lists `r01`–`r03`, the `1403cb100` disassembly `s01`, and the store scan `sc01`) |
| Verdict | PLAN. The values below are read from the exe. No gameplay check has run (NOT RUN). |

## Why it feels wrong

The tuning numbers Sora loads are already right. Walk 2, run 8, jump height 185, turn 0.2618 rad and fall cap 16 match what the exe loads. The feel is off because the **integrators** around those numbers were invented for the test room:

1. **Jump arc.** The exe has no launch velocity and no constant-gravity rise. It places Sora on a closed-form curve: a double-sine ease up to the apex over `T = √(2h/g)` ticks, then a parabola down with `g = 0.40816328`. Rust instead launches at 17.6 units per tick and subtracts a made-up 0.8 every tick. The result is about 23 ticks up and 21 ticks down, where the game takes 30 up and 30 down. Rust's jump is too short and too snappy.
2. **Air control.** The exe keeps momentum. Each tick, air velocity moves 10% of the way toward the stick velocity, and it shrinks by ×0.95 when the stick is released. Rust drives Sora along his facing at full speed every tick.
3. **Ground direction.** The exe moves Sora along the **stick** direction at once, while the model turns toward it at 15° per tick. Rust moves Sora along the **facing** direction, so every turn becomes a wide arc.
4. **Knock-back.** The exe decays every knock-slide by ×0.9 per tick and stops it below 1 unit per tick. Rust's hurt slide decays by ×0.8 and stops below 0.001, which covers about half the distance (100 units instead of 190 for a Shadow claw).
5. **Blend times.** The exe blends locomotion and fall motions over 8 frames. Rust uses 6.

## Conventions

- Tags: **[O]** confirmed in the exe (decompiled, and disassembled where noted); **[D]** read from installed data at runtime; **[I]** inferred from code shape, not proven; **[U]** not traced.
- The exe uses **Y-down**: a negative Y velocity moves up. Rust is Y-up, so signs flip when translating.
- `dt` is `DAT_140717480`, statically 1.0 [O]. The runtime writer (vsync interval × game speed) was not captured, so "1.0 per 60 Hz tick at normal speed" is [I]. The existing AI spec reached the same conclusion.
- `obj+X` is an offset in the shared actor object. `SSTM+X` is `DAT_142ae5760 + X`, the `03system → pref → sstm` record set by `FUN_1403e9360` [O].

## Where the tuning values come from

`FUN_14040a440` (object init) writes defaults into `obj+0x128..` [O]:

| Field | Default (bits) | Meaning |
| --- | --- | --- |
| +0x128 | 2.0 (`0x40000000`) | walk speed |
| +0x12c | 8.0 (`0x41000000`) | run speed |
| +0x130 | 160.0 (`0x43200000`) | jump height |
| +0x134 | 0.19635 (`0x3e490fdb`, π/16) | turn rate, rad/tick |
| +0x138 | `SSTM+0x14` (FallMax, 16 [D]) | downward speed cap |
| +0x13c | 0 | vertical move speed for fly/swim (read by `FUN_1403aa5d0`) [I] |
| +0x140 | 0.95 (`0x3f733333`) | [U] (overwritten by PRTY word 10) |

`FUN_1403da940` then calls `FUN_14040a480(obj+0x128, objentry+0x4e)` when the objentry's PRTY index is nonzero. That call copies PRTY record words 0–3 to +0x128..+0x134, word 8 to +0x13c and word 10 to +0x140 [O]. `FUN_1403e9590` resolves the index (an out-of-range index falls back to record 1) [O]. Sora's record gives walk 2, run 8, jump 185 and turn 0.2617994 [D] (logged by the test room). +0x138 is not overwritten, so it stays SSTM FallMax.

**Rust check needed:** `test_room.rs:1158` takes the PRTY record from a config `prty_pointer`. Step 1 makes it follow Sora's objentry `+0x4e`, with record 1 as the fallback, the way the exe does.

## Physics modes

`FUN_1403b4da0(obj, mode)` calls the old mode's exit slot (+0x10), then the new mode's enter slot (+0x08), and stores the mode at `obj+0xc` [O]. The mode objects are `PTR_PTR_140750d30[mode]` (`FUN_140409d20`) [O]. The slots this plan uses:

| Mode | Use [I from behaviour] | +0x08 enter | +0x30 land | +0x38 tick | +0x40 velocity | +0x48 jump/air action |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | ground | — | — | `140422520` motion pick | `140422350` | `140422630` start jump |
| 1 | jump / fall | `1403cbd30` | `1403cb940` | `1403cb680` | `1403cb360` | `1403cb9f0` aerial dodge |
| 4 | blow (class-4 reaction) | `1404228b0` | `1404228a0` | `1403cb680` | `140422780` | `1403cb9f0` |
| 8 | ground blow slide | `140410460` | — | `140410390` | `140410330` | — |
| 12 | [U] fly-blow-like | `1403f8950` | `1403f88c0` | `1403cb680` | `1403f8500` | `1403cb9f0` |

`FUN_1403b4780` is the per-tick object step. It runs the mode's +0x38 tick, applies the facing turn, and handles leaving the ground [O].

## Value table

Each row gives what the exe does, its evidence, what Rust uses now and where, and the gap. "Rust" paths are relative to the repo.

### Locomotion

| # | Value | Exe | Evidence | Rust now | Gap |
| --- | --- | --- | --- | --- | --- |
| L1 | Walk speed | PRTY w0 → +0x128 = **2.0** units/tick | `FUN_14040a480`, `FUN_1403aa5d0` [O]; value [D] | `Params::walk_speed` 2.0 from PRTY w0, `test_room.rs:1170` | Matches. Make record selection objentry-driven. |
| L2 | Run speed | PRTY w1 → +0x12c = **8.0** | same [O]/[D] | `run_speed` 8.0, `test_room.rs:1171` | Matches. |
| L3 | Walk/run threshold | Stick magnitude **≥ 0.8** (`DAT_140585c7c` = `0x3f4ccccd`) picks run, otherwise walk | `FUN_1403a8fa0` / `FUN_1403aa5d0` [O] | `WALK_THRESHOLD` 0.8, `test_room.rs:335`, plus a ±0.05 hysteresis band in `player_sim/src/lib.rs` (`tick`, walk/run pick) | The hysteresis is invented. Remove it: the exe compares `0.8 ≤ mag` with no band. |
| L4 | Acceleration / deceleration | **None.** Velocity `obj+0x20` = stick dir × speed the same tick; zero stick gives zero velocity | `FUN_140422350` (mode 0 velocity) [O] | Instant, `lib.rs` grounded branch | Matches (instant). |
| L5 | Move direction | The **stick** direction (`obj+0x10`, camera-relative from `FUN_1403a7fb0`), normalised | `FUN_140422350`, `FUN_1403a8fa0` [O] | Moves along **facing** (`self.facing()` after `turn_toward`), `lib.rs` grounded branch | **Gap.** Move along the stick direction; the facing turns separately. |
| L6 | Turn rate | PRTY w3 → +0x134 = **0.2617994 rad/tick** (15°). The facing `obj+0x68c` steps toward the desired yaw `obj+0x7a8` by at most `+0x134 × dt` | `FUN_1403b4780` → `FUN_1403b74c0` (clamped angle step) [O] | `turn_speed` 0.2618, `turn_toward`, `lib.rs` | Rate matches. That the desired yaw `+0x7a8` is the stick direction while moving is [I]: its writer was not traced. |
| L7 | Walk/run motion choice | Speed ≥ (walk + run) × 0.5 (`DAT_140623a4c` = 0.5) → RUN (2), else WALK (1); zero speed → IDLE (0) | `FUN_140422520` [O] | Picked from stick magnitude vs 0.8 | Equivalent while speeds are 2/8. Translate as written. |
| L8 | Locomotion blend | **8** frames (`DAT_140623bac` = 8.0) | `FUN_140422520` → `FUN_1403c6dc0` [O] | `BLEND_TICKS` 6, `test_room.rs:339` | **Gap.** Use 8. |
| L9 | Body radius, height, step height, floor slope | [U] (collision not traced) | — | 30 / 140 / 24 / 0.5, `test_room.rs:333–338` | Remain test-room choices. Out of scope here. |

### Jump and fall

The jump curve, translated from `FUN_1403cbb00` (start), `FUN_1403cbd30` (mode 1 enter) and `FUN_1403cb500` / `FUN_1403cb360` (per tick):

```
start (height h ≥ 0, horizontal vector v with speed w):
    t = 0; T = sqrt(2h / g); base = y_at_start; span = -h        // +0xf0, +0xf4, +0xf8, +0xfc
    air_velocity = normalize(v.xz) * w                            // +0xe0, copied to +0x20 on enter
each tick:
    t += dt
    if t < T: target = base + span * sin(π/2 * sin(π/2 * t/T))     // double sine: disassembly at 1403cb1d5/1403cb1ea
    else:     target = base + span + g/2 * (t - T)^2
    dy = target - y;  dy = min(dy, fall_max * dt)                  // Y-down: only downward motion is capped
```

| # | Value | Exe | Evidence | Rust now | Gap |
| --- | --- | --- | --- | --- | --- |
| J1 | Gravity | **g = 0.40816328** (`DAT_1405ce184` = `0x3ed0fac7`) | `FUN_1403cbb00`, `FUN_1403b73f0`, `FUN_1403cb500`, `FUN_1403b8150`, `FUN_140422780` [O] | `GRAVITY` 0.8 "test-room choice", `test_room.rs:332` | **Gap.** Use 0.40816328. |
| J2 | Jump height | PRTY w2 → +0x130 = **185**. With High Jump equipped, the FMAB level row's word 0 replaces it and the motion becomes 201 | `FUN_140422630` → `FUN_1403b59b0` [O]; value [D] | `jump_height` 185, `test_room.rs:1172` | Matches. |
| J3 | Rise curve | `y = h · sin(π/2 · sin(π/2 · t/T))` over **T = √(2h/g) = 30.11 ticks** for h = 185. Initial rise ≈ 15.2 units/tick; heights at t = 5/10/15/20 are 72.9/130.5/165.5/180.8 | `FUN_1403cb500`, constants `DAT_140623b78` = π, `DAT_140623a4c` = 0.5 [O] | Launch `√(2gh) + g/2` = 17.6, then `v -= 0.8` (`launch_speed`, `airborne`, `lib.rs`). Apex at tick 23 | **Gap.** Replace with the closed-form curve. |
| J4 | Fall curve | `y_apex − g/2 · k²`, k ticks past the apex. From 185 it lands at t ≈ 60.2 (≈ 1.0 s) at 12.3 units/tick, so the cap is not reached on flat ground | `FUN_1403cb500` [O] | Lands at tick 44 | **Gap.** Fixed by J1 and J3. |
| J5 | Fall-speed cap | +0x138 = SSTM FallMax **16** units/tick, downward only: `min(dy, 16·dt)` | `FUN_14040a440`, `FUN_1403cb500` [O]; 16 [D] | `fall_max` 16 applied to velocity, `lib.rs` | Matches. Apply it to the target delta instead of a velocity. |
| J6 | Walking off a ledge | Enter mode 1 with `t = vy/g`, `base = y − g/2·t²`; this continues the parabola from the current vertical speed (0 on flat ground) | `FUN_1403b8150`, `FUN_1403b4780` [O] | `vertical_speed = 0`, then `−0.8` per tick | **Gap.** Use the closed form with g. |
| J7 | Takeoff horizontal speed | Stick dir × current move speed (`obj+0x10`, `w = +0x1c`: 2 walk / 8 run / 0 still), stored at +0xe0 and copied to velocity +0x20 on enter | `FUN_140422630`, `FUN_1403cbb00`, `FUN_1403cbd30` [O] | None (air movement is recomputed each tick) | **Gap.** See A1. |
| J8 | Jump motion | JUMP (3); fall = 4, land = 5 (`+0x118 = 4`, `+0x11c = 5`) | `FUN_140422630`, `FUN_1403b59b0` [O] | Same IDs, `LOCOMOTION`, `test_room.rs:323` | Matches. |
| J9 | Switch to FALL motion | Plays FALL (4) blended over **8** frames once `T ≤ t + 5` (5 ticks before the apex), when the jump requested auto fall/land (+0x124 bit 0) | `FUN_1403cb680`, `FUN_1403cb610`; constants `DAT_140623b98` = 5, `DAT_140623b9c` = 6, `DAT_140623bc0` = 15 [O] | Fall starts when `vertical_speed ≤ 0` (apex); blend 6 | **Gap.** The constants are [O]. The branch that switches early (bit 0x40000 of +0x120, `6 < t < T−5`) is [I]. |
| J10 | Landing | Mode 1 land slot: plays LAND (5) with **blend 0** and switches to mode 0 | `FUN_1403cb940` / `FUN_1403cb2a0` [O] | `Action::Land` | Blend 0 matches an instant start. |
| J11 | Landing recovery | No fixed lock. After landing, mode 0's motion pick (`FUN_140422520`) changes to walk/run (blend 8) as soon as the stick moves, gated by `obj+0x18c` bits 2/4. Standing still lets LAND play out | [O] for the code. That those bits mean "can move" is [I] | Land holds until its ANB "blend to idle" unless moving | Close. Gate on the same bits once the ANB trigger → `+0x18c` mapping is traced (step 7). |
| J12 | Holding jump | No variable-height jump found: the height is fixed when the jump starts | `FUN_140422630`, `FUN_1403cbb00` [O for the path read] | None | Matches. Absence elsewhere is [U]. |
| J13 | Jump take-off delay | JUMP 03A carries a frame-35 trigger id 0. Whether it delays leaving the ground is [U] | combat-spec, ai-spec | Instant | Open. Trace in step 7. |

### Air control

| # | Value | Exe | Evidence | Rust now | Gap |
| --- | --- | --- | --- | --- | --- |
| A1 | Air steering | With stick: `vel = 0.9·vel + 0.1·(stick_dir × w)` per tick (`FUN_140141ae0` is lerp; t = `DAT_1405aaa40` = **0.9**). Displacement is `vel·dt`. About 22 ticks to reach 90% of the stick velocity | `FUN_1403cb360` → `FUN_1403b7ad0` [O] | `run_speed × max(mag, 0.8)` along the facing, every tick (`airborne`, `lib.rs`) | **Gap.** Translate `FUN_1403b7ad0`. |
| A2 | Air drag, stick released | `vel *= 0.95` per tick (`DAT_1405b1c74` = `0x3f733333`), a half-life of 13.5 ticks | same [O] | Velocity drops to 0 at once | **Gap.** |
| A3 | Air speed | `w` is walk 2 or run 8 by the 0.8 threshold, the same input function as the ground [I: the input slot runs while airborne] | `FUN_1403a8fa0` [O]; airborne use [I] | `run_speed × mag` | **Gap.** |
| A4 | Air turning | Facing turns at +0x134 in every mode (`FUN_1403b4780`) [O]. The airborne desired yaw is [I] | [O]/[I] | `turn_toward` while not attacking | Keep; confirm the desired-yaw writer. |
| A5 | "No gravity" ANB range (trigger 3) | While `obj+0x18c & 0x1000020` or bit 6 is set, the vertical delta is 0, but `t` still advances. When it clears, the curve resumes later and the fall cap limits the catch-up | `FUN_1403cb500` [O]. That trigger 3 sets those bits is [I] | Sets `vertical_speed = 0` and resumes from 0 | **Gap.** Keep `t` running; trace the trigger → bit mapping. |
| A6 | Aerial attack lift ("hop") | PTYA attack start `FUN_1403d78d0`. With PTYA+0xc > 0, it jumps toward the target: height clamped to [PTYA+0x14, PTYA+0x10 (+bonus)], horizontal speed = distance / `√(2h/g)` clamped to [PTYA+0x18, PTYA+0x1c], through the mode's jump slot (the same closed-form curve). Otherwise, if grounded, it lunges: speed = `(dist − reach + PTYA+0x3c) × SSTM+0xa8`, clamped to [PTYA+0x18, PTYA+0x1c] | [O] for the code; field labels [I]; SSTM+0xa8 value [D] | `lift(hop)` sets `vertical_speed ≥ launch_speed`; ground lunge is root motion only | **Gap.** Port in step 6. |

### Knock-back, hit reactions and blow

| # | Value | Exe | Evidence | Rust now | Gap |
| --- | --- | --- | --- | --- | --- |
| K1 | Knock-slide speed | `obj+0xd30 = (KB1 + target bonus +0x1c8) × 0.1` (`DAT_1406239f8`). A negative KB reverses the direction (× −1, `DAT_140623c48`). Direction `obj+0xd24` is normalised | `FUN_1403d4960` [O] | Guard recoil 200 × 0.1 (`GUARD_KNOCK`, `lib.rs`); hurt `kb × 0.1` (`test_room.rs:4185`) | Speeds match. Add the bonus field when equipment exists. |
| K2 | Knock-slide decay | Each tick: move `dir · v · dt`, then `v *= 0.9^dt` (`DAT_1405aaa40`). It stops when **v < 1.0** (`DAT_140623a80`). A Shadow claw (200 → 20/tick) slides 29 ticks and 190 units | `FUN_1403d4570` [O] | Guard: ×0.9, stop < 1, matches (`KNOCK_DECAY`). **Hurt: `HURT_SLIDE_BRAKE` 0.8, stop < 0.001** (`lib.rs`), 100 units | **Gap.** Route hurt through the same knock-slide. Shadow flinch uses `SLIDE_BRAKE` 0.8 (`shadow.rs:100`): same gap. |
| K3 | Flinch amount | KB1 0x7fff → SSTM HitBackSmall (+0x34) **200** for class 3, else HitBack (+0x30) 200; ×0.5 for reactions 12/13 | `FUN_1403ceab0` (combat-spec) [O]; 200 [D] | `test_room.rs:4185` hard-codes 200 | Read SSTM +0x30/+0x34 and apply ×0.5. |
| K4 | Blow start | Velocity `+0x20` = horizontal hit dir × KB2, `+0x24 = −KB1` (up). Defaults when both are 0: SSTM BlowUp +0x24 = 24, BlowSpeed +0x28 = 40. Then `FUN_140409d30` picks mode 4 (or 8/0x21/0x2d/0x15 by state) | `FUN_1403d2fe0`, `FUN_140409d30` [O]; values [D] | `shadow.rs` Blow: velocity `[dir·KB2, KB1, …]` | Matches at the start. |
| K5 | Blow flight (mode 4) | Enter: `+0x10c = |v_xz| × SSTM+0x1c` (BlowMinXZ **0.1**). Each tick: `|v_xz| *= SSTM+0x18` (BlowBrakeXZ **0.8**), floored at +0x10c. If `vy < 0` (rising): `vy *= SSTM+0x20` (BlowBrakeUp **0.8**). Then `vy = min(vy + g, fall_max)` | `FUN_1404228b0`, `FUN_140422780` [O]; values [D] | `velocity[1] -= GRAVITY` with no up-brake; `BLOW_BRAKE_XZ` 0.8, `BLOW_MIN_XZ` 0.1 used as constants | **Gap.** Translate `FUN_140422780`, reading SSTM. |
| K6 | Blow land | Mode 4 land slot: motion = blow motion + 1 (17/19), blend 0, then mode 0 | `FUN_140422700` [O] | Same motions | Matches. |
| K7 | Ground blow slide (mode 8) | `vel *= SSTM+0x3c` (FlyBlowBrake 0.8) per tick; below +0x10c it plays the stand motion and switches to mode 5 (or 0) | `FUN_140410330`, `FUN_140410390` [O] | Not modelled | Add when a state enters mode 8. |
| K8 | Hit-stop | None found on the hit path | combat-spec §2.5 [O for the path read] | None | Matches. Camera shake and global slow-down are [U]. |
| K9 | Hitstun length | Damage motions 10–13 play; the point Sora can act again was not traced | [U] | Hurt ends at clip end (`lib.rs` `Action::Hurt`) | Open (step 7). |
| K10 | Shadow jump claw | `method_obj_jump` → `FUN_1403cbb00` with height 100 and speed 5: the same closed-form curve as J3 | ai-spec, `FUN_1403cbb00` [O] | `shadow.rs:681`: launch `√(2gh)` with linear gravity | **Gap.** Share the J3 integrator. |

### Abilities (not equipped in the test room)

The growth level is `FUN_140401680(table, n)`, byte n of a level table, forced to 5 by a debug/override check [O]. The rows come from FMAB (`FUN_1403e92d0`) [O]. The labels follow the call sites [I]:

- n = 0: High Jump. Jump height comes from row word 0; motion 201 (`FUN_1403b59b0`).
- n = 1: Quick Run. Speed is row +0x10; +0x14/+0xc/+0x18 go to +0x108/+0x10c/+0x110; motion 203; mode 0x10 (`FUN_1403a96c0`, `FUN_1403a9830`).
- n = 2: Aerial Dodge. Height is row +4, speed row +8; motion 202 (`FUN_1403cb9f0`).
- n = 3: Glide. Requires ground clearance above row +0x34; mode 0x11/0x12 (`FUN_1403a81f0`).
- n = 4: Dodge Roll. Motion 209; row +0x40 → +0xd70 (`FUN_1403a81f0`).

Sora at level 1 has none of these, so they are out of scope until growth abilities exist.

## Implementation steps

Each step keeps the translated routine's structure and constants and names the original address in a comment. Each step adds synthetic tests and runs `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo build --workspace --locked` and the smoke run, plus one autoplay run compared with `autoplay-76`.

1. **Movement parameters from the exe's own sources.** Add `player_sim::physics::ObjParams`, translated from `FUN_14040a440` + `FUN_14040a480`: defaults 2/8/160/π/16, `fall_max` = SSTM+0x14, PRTY words 0–3/8/10 when objentry +0x4e ≠ 0, record 1 as the fallback. Read the PRTY index from Sora's objentry instead of `config.prty_pointer`. Delete the `GRAVITY` test-room constant and use `G = 0.408_163_28` (`DAT_1405ce184`).
2. **Closed-form jump/fall integrator.** Translate `FUN_1403cbb00` / `FUN_1403cbc30` (start), `FUN_1403cbd30` (enter), `FUN_1403cb500` (per tick, with the double sine), `FUN_1403b8150` (ledge fall) and `FUN_1403b73f0`. Player state gains `t`, `T`, `base`, `span`. Convert to Y-up at the boundary. Use it for Sora's jump and ledge fall and for the Shadow's jump claw. Tests: apex 185 at t ≈ 30.1, landing at t ≈ 60.2 on flat ground, cap 16 on long falls.
3. **Ground velocity along the stick.** Translate `FUN_140422350`: displacement = stick dir × speed. Keep the facing turn as `FUN_1403b74c0` at +0x134 per tick. Remove the walk/run hysteresis and pick the motion with `FUN_140422520` (`speed ≥ (walk + run)/2`). Set the locomotion blend to 8.
4. **Air momentum.** Translate `FUN_1403b7ad0` with (dt, 0.9, 0.95) for mode 1: take-off velocity from J7, 10% lerp toward the stick, ×0.95 with the stick released. Keep `t` advancing through no-gravity windows (A5).
5. **Fall motion timing.** Translate `FUN_1403cb680`'s fall-motion switch (`T ≤ t + 5`, blend 8) and the land slots `FUN_1403cb940` / `FUN_1403cb2a0` (LAND, blend 0, mode 0).
6. **Knock-slide and blow.** Route Sora's hurt slide and the Shadow flinch through `FUN_1403d4960` / `FUN_1403d4570` (0.9 decay, stop < 1), deleting `HURT_SLIDE_BRAKE` and `SLIDE_BRAKE`. Read HitBack/HitBackSmall from SSTM with the ×0.5 rule. Translate blow mode 4 (`FUN_1404228b0`, `FUN_140422780`, `FUN_140422700`) for the Shadow, reading SSTM +0x18/+0x1c/+0x20. Port the PTYA attack-move start `FUN_1403d78d0` for aerial hops and ground lunges.
7. **Close the remaining [I]/[U] items in Ghidra before calling the feel 1:1.** These are: the writer of the desired yaw `+0x7a8`; the ANB trigger → `obj+0x18c` bits (no gravity, can move, actionable) that gate landing recovery and hitstun; the JUMP 03A frame-35 trigger; the early fall-switch flag `+0x120` bit 0x40000; the runtime value of `DAT_140717480`; and collision radius, height and step (`L9`). Record each in a task file with [O] evidence.
8. **Side-by-side check.** Run a scripted jump, a run-and-turn, a ledge fall and a Shadow claw hit in the rewrite. Log per-tick position, and compare against the curves above and, if the user provides one, a capture from the installed game. Nothing claims 1:1 until this step passes.

Steps 1–6 can land as one change each, in order. Step 7 can run in parallel with steps 3–6.

## Implementation status

Branch `claude/project-thread-dhv9n6`, one commit per step. Evidence runs go in `research\playable-room-20261008\movement-01\autoplay-sN`.

| Step | State | What changed (value, source) |
| --- | --- | --- |
| 1 | Done | `player_sim::physics::ObjParams` (FUN_14040a440 defaults, FUN_14040a480 PRTY copy), `prty_offset` (FUN_1403e9590), `Sstm` (SSTM +0x14..+0x34). The room now picks the PRTY record from the player objentry's +0x4e (`P_EX100` → index 1, offset 284; it was config `prty_pointer` 1 before, so the values are unchanged: walk 2, run 8, jump 185, turn 0.2618, fall max 16). Gravity is G = 0.40816328 (`DAT_1405ce184`) in place of the test-room 0.8. `autoplay-s1`: 7 hits, as in `autoplay-76`. Captures after tick 594 came out black because the display went dark (window not drawn), and the run exited 1 for that reason only. |
| 2 | Done | `physics::Arc`: FUN_1403cbb00/FUN_1403cbc30 launch, FUN_1403cbd30 base, FUN_1403cb500 per-tick target (double-sine rise over √(2h/g), parabola fall, downward delta capped at FallMax 16), FUN_1403b8150/FUN_1403b4780 ledge fall. Sora's jump (h 185: apex 185 at t 30.1, lands on tick 60), ledge falls and aerial-attack hops (a new arc of the PTYA Jump height, FUN_1403d78d0) use it; ANB no-gravity ranges hold the height while `t` runs. The Shadow's jump claw uses it with h 100. The Jump→Fall action switch is still at the apex until step 5. |
| 3 | Done | Ground displacement follows the stick direction at the full walk/run speed (FUN_140422350) while the facing turns at 0.2618 rad/tick (FUN_1403b74c0); any stick past the dead zone moves. Walk/run picks at magnitude 0.8 with no hysteresis (FUN_1403a8fa0). Walk↔run keeps the gait phase, `fmod(len_new·(t + 8)/len_old, len_new)` (FUN_1403c6dc0). Blends per action (`player_sim::blend_for`): 8 for idle/walk/run/fall (`DAT_140623bac`), 0 for jump, land, attacks, damage, guard and recoil; after an attack ends, its PTYA +0x38 blend. |
| 4 | Done | Airborne horizontal velocity (obj+0x20): take-off = stick direction × walk/run speed (FUN_140422630 → FUN_1403cbb00 +0xe0, copied by FUN_1403cbd30); ledge drops keep the ground velocity [I: +0xa38]. Each tick FUN_1403b7ad0 with (0.9 `DAT_1405aaa40`, 0.95 `DAT_1405b1c74`): v = 0.9·v + 0.1·stick velocity, or v ×0.95 with no stick; aerial attacks take no stick [I]. Facing still turns at 0.2618 rad/tick. The Shadow's jump claw drags ×0.95 from speed 5 [I: no move request]. |
| 5 | Done | Jump → Fall action (FALL motion 4, blend 8) once `rise ≤ t + 5` (FUN_1403cb680 / FUN_1403cb610, `DAT_140623b98` = 5): tick 27 of a 185 jump, 3 ticks before the apex. Landing starts LAND with blend 0 (FUN_1403cb940). Landing recovery still waits for LAND's ANB "blend to idle" unless the stick moves; the obj+0x18c gate (J11) stays open for step 7. |
| 6 | Done | `physics::Knock` (FUN_1403d4960 / FUN_1403d4570: strength × 0.1, ×0.9 per tick, stops below 1) now drives Sora's hurt slide, his guard recoil, the Shadow's flinch and its reflect, replacing the 0.8 brakes; a Shadow claw (KB1 0x7fff) slides Sora 190 units over 29 ticks. Strength comes from SSTM HitBackSmall/HitBack with ×0.5 for reactions 12/13 (`Sstm::knock_strength`, FUN_1403ceab0). The Shadow's blow is mode 4 (`physics::Blow`, FUN_1404228b0 / FUN_140422780): XZ ×0.8 down to a floor of 10% of the start speed, rising speed ×0.8, then +g capped at FallMax; KB1/KB2 both 0 fall back to SSTM BlowUp/BlowSpeed. Aerial hops take the PTYA +0x1c speed along the facing (FUN_1403d78d0, no-target branch; installed air steps: Jump 21/17/21, speed 8/8/18). Not done: the targeted hop and the ground lunge (FUN_1403d78d0 with a target, which needs FUN_1403d7520). |

Checks after step 6: PASS fmt, clippy, workspace tests (36 in `player_sim`), build, smoke (`--frames 180`), and `autoplay-s6` (arena-07: exit 0, 81 non-uniform captures, Shadow hits at 417/448/486, jump on tick 300 → FALL 327 at 185.0 → LAND 361). `autoplay-s3` (after step 3) also exited 0. A hands-on play check with a pad and a side-by-side against the installed game are NOT RUN.

Still open (step 7): the targeted hop and ground lunge, the desired-yaw writer, the ANB trigger → obj+0x18c bits (landing recovery, hitstun), the JUMP 03A frame-35 trigger, and the collision radius, height and step.

### Second pass (2026-10-10)

The user reported the feel still differed. Tracing the position integrator FUN_1403b89a0 (called from the per-object update FUN_1403bfd30) found that a physics mode's velocity routine does not run every frame. It runs when the timer obj+0xba8 drops below 0, every `DAT_1405b18ec` = 2.0 frames (every frame only for move requests faster than `DAT_140623be4` = 100), with that 2-frame span as its time step. The result divided by 2 is stored at obj+0xb98 and applied each frame × frame time. Root motion (FUN_1403b7ca0), the knock-slide (FUN_1403d4570), slope slide (FUN_1403b81d0) and force fields (FUN_1403f9130) are added every frame. Collision (FUN_1403b9090) follows. FUN_1403b4da0 clears the timer on every mode change.

`physics::Sampler` translates this. Ground movement, the jump curve (t advances 2 per run), air steering (0.9/0.95 once per 2 frames) and the Shadow's blow (gravity and brakes once per 2 frames) now run through it. Frame time is confirmed: FUN_140154740 sets `DAT_140717480` to the vsyncs elapsed × game speed (`DAT_140717424` = 1.0), so it is 1.0 per 60 Hz frame. FUN_140154600 doubles it only in 30 fps mode. Sora's locomotion, jump, fall, land and hurt clips carry no root motion (logged as `root_start`/`root_end`), so velocity alone moves him there.

Checks: PASS fmt, clippy, workspace tests (37 in `player_sim`), build, smoke, and `autoplay-s7` (exit 0, 81 non-uniform captures; jump at 300 → FALL 326 at 184.9 → LAND 361).

### Third pass (2026-10-10): jump take-off, landing lock, aerial hops

The user reported that jump/fall and air combos felt off, and that the rewrite let Sora move again instantly after landing.

- **Jump take-off [O].** Pressing jump queues the jump mode (FUN_1403cbb00 sets obj+0x648 = 1). The switch to mode 1, with the arc base and the stored take-off velocity, happens when the JUMP motion's frame trigger 0 fires (FUN_14040dd70, case 0 → FUN_1403b4da0(1)). On Sora's JUMP that is tick 4, so he crouches for 4 frames before lifting off.
- **Actionable gating [O for commands, I for the stick].** Commands are taken only while an ANB range 0, 1 or 2 is active (FUN_14040d810 maps them to obj+0x18c bits 2/3/4; FUN_1401b15f0 checks `& 0x1c`). Installed ranges:
  - idle/walk/run/fall: always;
  - JUMP: from tick 8;
  - LAND: from tick 12;
  - damage 10–13: from tick 50;
  - guard: from tick 52;
  - recoil: from tick 40;
  - aerial attacks: none.

  The stick feed to the move request follows the same gate [I: its own gate was not traced; the user's report of the original's landing lock supports it]. LAND therefore locks movement and actions for 12 frames, then plays to tick 26 unless the stick moves. Damage now recovers at tick 50 instead of 70.
- **Range 42 [O].** It maps to obj+0x190 bit 0. FUN_140422350 then moves mode 0 through FUN_1403b7ad0 with PRTY record +0x28/+0x2c (words 10/11) as inertia and drag. Sora's record holds 0/0, so the carried velocity stops dead on landing.
- **Aerial hops toward the target [O formula; I radii].** FUN_1403d78d0, with a target:
  - height = target rise clamped to [PTYA +0x14, +0x10];
  - speed = (horizontal distance − FUN_1403d7520's summed radii + PTYA +0x3c) / √(2h/g), clamped to [+0x18, +0x1c], toward the target;
  - then FUN_140409ad0 (FUN_1403cbb00 + mode 1).

  The test room uses 2 × Sora's body radius for the summed radii.
- **Turning toward the target [O].** ANB range 28 turns by up to its parameter in degrees per tick; frame trigger 21 turns at once (FUN_1403d4b10 / FUN_1403d3a90, π/180 = `DAT_1405b15b4`), only beyond 50 units. Installed values:
  - ground combo: 15° for ticks 0–12;
  - air steps 1–2: 15° for ticks 0–23 / 0–22;
  - finisher: 360° at tick 0.

Checks: PASS fmt, clippy, workspace tests (40 in `player_sim`), build, smoke, and `autoplay-s8` (exit 0, 81 non-uniform captures; jump pressed 300, lift-off 304, FALL 329, LAND 364, idle 390).

