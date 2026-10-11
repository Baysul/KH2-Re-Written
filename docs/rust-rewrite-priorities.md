# Essential Rust rewrite priorities

Reviewed 2026-10-10 against checkout `f248223384d5012c69739bce39d5a12c77834e99`.

This is a ranked implementation backlog for **KH2 Final Mix only**. Importance means how much a system blocks a faithful, playable, recoverable game. Lower-ranked systems are still required for the full-game goal. Dependencies can require a small part of a later item earlier.

The project is already written largely in Rust. The work is to finish and validate the existing implementation, not rewrite working Rust again. Original assets remain in the user's read-only Steam installation; Rust implements their readers and runtime behavior.

## Most important to least important

### 1. Authoritative game state, timing, input and reproducible comparison

**Why first:** movement, combat, scripts, animation and saves all depend on the same state and update order. Errors here propagate everywhere.

- **Exists:** `player_sim`, `area_sim`, `event_sim`, and fixed-tick player logic. Some state and gameplay orchestration still live in `windows_shell`.
- **Essential work:** establish measured clocks, input sampling, pause behavior, update order and RNG consumption; keep authoritative state independent of rendering; record timestamped inputs and compare replay state at the first divergence. Do not assume that a 60 Hz global frame implies every subsystem updates at 60 Hz.
- **Completion evidence:** repeated identical inputs produce identical discrete state; installed-reference traces agree for each qualified scenario, with measured numeric/timing tolerances.
- **Gates:** P09, P11, P17. **Locations:** `crates/player_sim`, `crates/area_sim`, `crates/event_sim`, `crates/windows_shell/src/bin/kh2fm/test_room.rs`.

### 2. Supported-source admission and dependable data decoding

**Why:** every feature needs the correct game records, and every gameplay session must require a validated supported source.

- **Exists:** `source_identity`, bounded readers, launcher checks and session-bound chunk verification. [T080](tasks/T080-launcher-and-session-check.md) describes what the fast path does and does not verify. The probe code still explicitly handles a quarantined, unqualified manifest.
- **Essential work:** finish build/edition/shared/language qualification; distinguish research admission from supported gameplay; audit all entry points and reads; reject changed or unsupported sources without accepting a remembered path or cache as proof. Extend readers only for actual needed formats, preserving unknown fields.
- **Completion evidence:** synthetic corruption and admission tests, independently checked decoded records, complete dependency identity, and refusal of unsupported builds. Add transactional source-bound caching when needed; cache never replaces session validation.
- **Gates:** P02, P06–P08, P12, P29. **Locations:** `crates/source_identity`, `crates/package_reader/src/probe.rs`, launcher install code.

### 3. Player movement, collision, camera and action transitions

**Why:** this is the moment-to-moment foundation of every playable room and fight.

- **Exists:** locomotion, jumps, follow camera, collision and newer movement passes; `field_collision` supplies shared collision behavior.
- **Essential work:** qualify ground/wall/edge response, slope and step handling, moving geometry, airborne control, landing locks, targeting camera and action interruption. Resolve remaining test-room choices from evidence.
- **Completion evidence:** measured walk/run/jump/land/attack-interruption routes and camera traces, including corners and room transitions; controller playtest.
- **Gates:** P16–P18. **Locations:** `crates/player_sim`, `crates/field_collision`. **Evidence:** [current state](CURRENT-STATE.md), [T090](tasks/T090-attack-selection.md).

### 4. Complete combat and enemy behavior

**Why:** a single-enemy arena is not enough to support campaign encounters.

- **Exists:** combos, a Shadow, guard, damage, hit reactions, magic, items, EXP and prize logic.
- **Essential work:** finish multi-enemy targeting, hit volumes and timing, attack interruption, reactions and reaction commands, enemy/boss state machines, difficulty rules, defeat/retry flow and remaining ability actions. Generalize from actual actor programs and records rather than duplicating the Shadow's behavior.
- **Completion evidence:** reference-backed encounter traces covering hits/misses, damage, invulnerability, resources, enemy decisions, death and retry.
- **Gates:** P19, P21. **Locations:** `crates/player_sim/{combat,shadow,targeting,magic,abilities}.rs`. **Evidence:** [T091](tasks/T091-hit-records-and-damage-terms.md) through [T096](tasks/T096-exp-prizes-drive-gauge.md).

### 5. Story scripts, event flags and world progression

**Why:** this turns separate demonstrations into a game that can be played from beginning to end.

- **Exists:** area scripts, spawn/exit logic, 42 Twilight Town areas and an event timeline. The current-state record says story state is fixed and all those areas are unlocked.
- **Essential work:** execute required script instructions/native services, persist story flags, enforce unlocks, select encounters and spawns, advance objectives, and connect battles, cutscenes and area transitions. Preserve unsupported script operations as explicit gaps.
- **Completion evidence:** a real route advances through exploration, an event, a battle and a changed world state without debug shortcuts.
- **Gates:** P16, P20–P22, P24. **Locations:** `crates/area_sim`, `crates/event_sim`, `crates/windows_shell/src/bin/kh2fm/area.rs`. **Evidence:** [T097](tasks/T097-twilight-town.md).

### 6. Native saves, loading and recovery

**Why:** progress must survive closing the application before a long campaign is useful.

- **Exists:** configuration persistence; current-state notes explicitly identify missing Load/Save and in-memory-only journal marks.
- **Essential work:** versioned authoritative-state serialization, slot selection, atomic writes, recovery, migration, and consistent restart/continue behavior. Cover progression, inventory, abilities, party, journal and settings as appropriate. Vendor-save import remains a separate undecided requirement.
- **Completion evidence:** save/reload preserves state; synthetic truncated/corrupt/version-mismatch cases fail safely; interrupted writes retain a recoverable save. Never overwrite vendor saves.
- **Gate:** P23. **Location:** introduce a persistence boundary when implementing actual state round trips; no empty crate needed now.

### 7. Party members, Drive forms, summons and Limits

**Why:** the full combat and progression model needs more than Sora alone.

- **Exists:** party menu and Drive gauge. Current-state notes say Donald/Goofy field behavior and entering Drive forms are not implemented.
- **Essential work:** companion spawning, movement/AI, attacks, healing, targeting, member configuration and guests; then form entry/exit, costs, selection rules, form EXP, motion/attack sets, summons and Limits, following reference evidence.
- **Completion evidence:** multi-actor encounters reproduce resource use and state transitions, including party changes and form restrictions.
- **Gates:** P19, P22, P27. **Locations:** `crates/player_sim/`, area actor integration; new modules only as behavior requires them.

### 8. Inventory, equipment, stats and growth as real game state

**Why:** menus and combat must use the same values and rules.

- **Exists:** item, equipment, ability and status screens; battle items, EXP and some level-up behavior.
- **Essential work:** recompute stats after equipment changes, implement weapon lists/form sub-weapons, ability constraints, full level growth, shops, synthesis, rewards and persistent unlocks. Wire presentation to authoritative transactions.
- **Completion evidence:** equip/use/buy/reward operations update combat and saved state consistently; reference cases cover limits and failure conditions.
- **Gates:** P22, P23, P25. **Locations:** `crates/player_sim`, `crates/windows_shell/src/camp`, decoded item/battle tables.

### 9. Complete animation and motion-event runtime

**Why:** animation also determines attack windows, movement and event timing, not just appearance. Implement those gameplay dependencies alongside ranks 3–4.

- **Exists:** skeletal evaluation, IK work, motion triggers and cutscene actor animation.
- **Essential work:** qualify remaining solver semantics, root/scale channels, blending, attachments, texture animation and mesh visibility; synchronize motion events with authoritative actions.
- **Completion evidence:** sampled reference poses, root trajectories and trigger times agree across representative action families; no claim of completeness from one motion.
- **Gates:** P15, P17, P18. **Locations:** `crates/package_reader/src/motion*.rs`, model pose/texture modules. **Evidence:** [unknowns U019/U022](unknowns.md), [T099](tasks/T099-axel-roxas-cutscene.md).

### 10. Cutscenes and transitions integrated with gameplay

- **Exists:** the Axel/Roxas scene with actors, camera, subtitles, audio and effects.
- **Essential work:** required event commands, skip/pause/resume behavior, ending transitions and correct progression effects. Integrate scenes into story execution rather than launcher-only demonstrations.
- **Completion evidence:** entering, completing and skipping each tested scene produce the expected state, audio and next location.
- **Gates:** P20, P24. **Locations:** `crates/event_sim`, `crates/windows_shell/src/bin/kh2fm/cutscene/`. **Evidence:** [T099](tasks/T099-axel-roxas-cutscene.md).

### 11. Complete worlds, encounters and special modes

- **Exists:** Twilight Town traversal and a small combat sample; no complete coverage ledger is established by those samples.
- **Essential work:** discover the complete KH2FM content denominator, then implement each world's unique actors, encounters, bosses, minigames, Gummi routes, cups and optional systems. Prioritize campaign dependencies before optional route expansion, while keeping both required.
- **Completion evidence:** tracked routes, prerequisites and discrepancies for every required entry, followed by full campaign and optional-content testing.
- **Gates:** P27, P28. **Locations:** area/script/combat runtime plus coverage ledgers.

### 12. Complete UI, journal, settings and both text options

- **Exists:** substantial title/camp/HUD/journal presentation, with incomplete pages and controls.
- **Essential work:** remaining journal pages, game-over/load/save/shop flows, controller prompts, settings, missing menu interactions, English/Japanese message/font coverage and layout. Build the UI required by ranks 6–8 alongside those systems.
- **Completion evidence:** every required screen is reachable and functional with keyboard/controller; both text options render available source resources correctly. Do not fabricate missing language assets.
- **Gate:** P25. **Locations:** `crates/windows_shell/src/camp/`, HUD/input/layout modules, message/font readers. **Evidence:** [T093](tasks/T093-journal-pages-notices.md), [T098](tasks/T098-item-pictures.md).

### 13. Rendering, effects and audio fidelity

- **Exists:** rooms/models, effects VM, music and sound playback, and partial cutscene effects.
- **Essential work:** replace spell/prize stand-ins, complete material/blend/order behavior, HD routing, map effects, animation-driven visuals, voices, loops and sound scheduling. Effects that communicate hits or hazards are needed earlier with combat.
- **Completion evidence:** controlled installed-reference image/audio comparisons under pinned settings; preserve known raster failures and untested behavior until resolved.
- **Gates:** P14, P24–P26. **Locations:** `crates/effect_vm`, `crates/audio_out`, shell rendering/effects. **Evidence:** [current state](CURRENT-STATE.md), [unknowns](unknowns.md).

### 14. Performance, installation changes and release reliability

- **Essential work:** profile loading/memory/frame pacing, bound caches, handle source changes and failures safely, test supported Windows/device configurations and audit package provenance/dependencies.
- **Completion evidence:** measured performance, clean-machine launch, safe recovery and a package containing only permitted code/assets. Publication requires separate user instruction.
- **Gates:** P29–P31. Optimize measured bottlenecks as they appear; final qualification needs the completed game.

## Recommended next milestone

Build one **reference-checked, saveable story segment**: validated session → correct spawn → movement → encounter → story flag change → next area → save → restart → load the same state. Combine ranks 1–6 for this narrow route before broadly adding more demonstration screens. Reuse existing implementations and keep each missing behavior explicit.

## Scope and evidence limits

- Ranking is engineering judgment based on repository evidence, not a new gate acceptance or a claim of installed-game equivalence.
- Sources: [fidelity contract](fidelity-contract.md), [milestones](milestones.md), [unknowns](unknowns.md), [current state](CURRENT-STATE.md), [architecture](../ARCHITECTURE.md), workspace modules and linked task records. The milestone/unknown summaries lag newer implementation records; newer implementation does not automatically resolve their fidelity requirements.
- Some existing records are dated 2026-10-11; those dates are retained as written. This review uses the user-facing date 2026-10-10.
- No fresh gameplay, installed-file inspection or public technical-source verification was performed. Implementation summaries describe repository claims and code presence, not newly verified behavior.
- Existing other-game side work is outside this KH2FM ranking. No existing files or side work need to be deleted to follow it.
- Offline research scripts do not all need porting to Rust. Prioritize the shipped runtime, bounded readers and reproducible verification infrastructure. Assets themselves are read, not rewritten or committed.
