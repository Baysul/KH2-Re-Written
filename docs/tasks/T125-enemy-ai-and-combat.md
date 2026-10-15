# T125 — Executable enemy AI, combat, lock-on and Scan

Owner Codex, 2026-10-14 through 2026-10-15. Gates P10/P12/P15/P19/P21/P27
development work; no final gameplay acceptance inferred. Prerequisites:
fidelity contract, milestones, unknowns, T124, source enemy scripts and existing
Shadow/player combat. User requests the complete enemy AI/combat roster plus
lock-on and Scan. HEAD `3aa251e`, package 0.1.1; preceding uncommitted work is
preserved. Selected KH2FM Steam app 2552430/build 15194255 English remains a
quarantined candidate; supported edition/profile is unqualified.

Private evidence root:
`%LOCALAPPDATA%/KH2FinalRewrite/research/enemy-ai-combat-20261014/`.
Source is selected and validated at each gameplay session. Source installation
and user saves remain read-only. Private backups are outside the checkout at
`%LOCALAPPDATA%/KH2FinalRewrite/backups/T125-20261015/`. No commit, push,
publication, installation update or vendor-save write is authorized here.

## Steps and implementation

1. Reuse the source-bound REA identity and read-only Ghidra project. Qualify
   native motion clocks, event ranges, stats, collision, jump/fly consumers,
   manual targeting, Scan, lock camera and attack identity lifetimes. Independently
   map every retained instruction to source bytes. Finite slices remain partial
   candidates; no complete logical function or installed behavior is inferred.
2. Translate ordinary Dusk and Soldier source decisions/actions alongside
   Shadow. Connect independent action queues, HP, damage/revenge/death state,
   movement and source attacks. Preserve unsupported branch faults. All scripts
   consume the shared script RNG stream; authored actor update order remains
   unqualified against the installed scheduler.
3. Load ENMP/LVPM/SSTM/ATKP/PRZT and actual HIT/TARGET/ATTACK/camera shapes from
   the selected source. Evaluate the current skeletal pose for contacts. Share
   Sora's attack pool, hit records, incoming/outgoing damage, guard/clash, spells,
   EXP and prizes across the summoned actors. Source literal combo groups stay
   global. Expired attacks clear seen bits and release IDs, preserving repeat
   timers and record locks. Removed actors also release their owned IDs.
4. Add stable actor/shape targeting. R1 promotes the existing automatic target
   or runs the native initial chooser; another press releases it. Directional
   cycling uses native signed angular selection in native coordinates. Invalid
   or untargetable actors release lock. Selected targets drive attacks, magic
   and the admitted lock-camera subset. Camera anchors prefer source kind-seven
   group zero, with target fallback.
5. Bind Scan to source item 138 / ability bit 19. Toggle only owned inventory
   slots and rebuild equipped abilities. The debug target panel shows live
   primary-pool HP only while Scan is equipped; it uses the source font but
   does not claim native HUD sprite/layout fidelity.
6. Exercise three simultaneous ordinary actors, source action progress, real
   damage both ways, equipped/unequipped Scan and manual lock/release in an
   unattended renderer tour. Preserve every failed/intermediate run. Run all
   mandatory workspace/progress checks before replacing the local launcher.

F3 → Summon enemies → Summoned actors → selected actor → Enable AI and combat
supports ordinary Shadow, Dusk and Soldier. Freeze, defeat and respawn remain
development controls. Other catalog entries retain animation inspection and an
explicit unsupported AI reason. The host currently requires the generated flat
arena; placements, no-leave context and clear step probes are authored debug
choices. This is not the full roster or source encounter activation.

## Exact evidence and boundaries

All paths below are relative to the private evidence root:

- `dusk/decision-oracle-01.json`: PASS 15,360 source-interpreter comparisons,
  656,482 executed script instructions; requests, RNG and counters checked.
- `soldier/decisions-oracle-02.json`: PASS 6,600 comparisons and 286,392 executed
  script instructions. Neither decision oracle executes native physics.
- `native/independent-source-01.json`: PASS 162 exploratory slices, 9,666
  instructions and 41,089 instruction bytes, independent PE mapping, source
  unchanged. Discovery/interior/rejected leads remain distinct in
  `native/public-map-review-01.json`.
- `dusk/independent-source-01.json`: PASS 40 exploratory slices, 1,630
  instructions / 7,108 bytes, independently source-matched. Actual fly physics
  is distinguished from the earlier incorrect physical-mode discovery lead.
- `native/contract-01.md`, `dusk/source-to-state-01.md`, source-bound native pass
  folders: motion clock/end pulse/loop/range, source curve-time origin,
  ENMP/weakness, collision, ordinary Arc, Dusk takeoff/fly/fall/land, air wish,
  manual choosers, Scan, camera subset, attack allocation/destruction contracts.
- `native/scan-name-01.json`, `native/source-lock-01/admission.txt`,
  `native/plyr-fields-01.json`: fresh Scan name/item/ability and target parameter
  bindings. Item identity and ability-bit identity are separate namespaces.
- `loader/pose-probe-02/report.json`, `loader/independent-comparison-02.json`:
  independently checked source stats/collision and finite Shadow/Dusk pose
  samples. Soldier admission is additionally exercised in the rendered tour.
- `clock-profile-01.json`: fresh three-MSET metadata inspection. Valid idle
  clips can omit an event BAR member; nonzero frame starts must retain their
  source curve-time origin. Neither is grounds to reject all combat clips.
- `public-id-map-final-01.json`: 148 distinct new PARTIAL native candidates,
  deduplicated against T124 and within T125, including 16 additional fly
  candidates. Only opaque IDs/status/public acceptance links enter Git.
- `combat-tour-01*`: preliminary earlier two-actor integration PASS; it predates
  final targeting and native clocks and is not final delivery evidence.
- `combat-tour-02*`: FAIL admission incorrectly required event members and zero
  frame starts; two later captures were 1×1/uniform. Those failures are retained.
- `combat-tour-03*`: all three actors enable and remain free of AI faults for
  1,800 ticks; six positive hits each direction, equipped/unequipped Scan and
  lock observed, all seven captures nonuniform. Overall FAIL: the scripted
  release was scheduled after native untargetability had already released the
  first lock, so the second toggle acquired a new lock. Correct the exercise's
  release timing rather than changing native target invalidation.

## Checks, uncertainty, verdict and next task

PASS `combat-tour-05.jsonl`, `combat-tour-05.exit.json` and seven nonuniform
captures under `combat-tour-05/`: three ordinary actors enabled, zero AI faults,
1,800 simulation ticks, twelve positive damage contacts to enemies, two to Sora,
one Soldier defeat with four EXP, Scan hidden at tick 100, equipped with manual
lock at tick 160, and manual release before tick 300. Captures 01 and 03 were
visually inspected for source actors, lock/Scan HP panel and an actual attack.
Earlier `combat-tour-04*` retains the same too-late lock checkpoint failure after
the shared RNG correction; production target invalidation was preserved.

PASS `cargo fmt --all --check`, warnings-denied locked workspace/all-targets
Clippy, locked workspace tests (556 passing executions), locked workspace build
and the 180-frame shell smoke. Evidence: `{fmt,clippy,tests,build,smoke}-delivery.log`,
`checks-delivery.json`, `tests-delivery-02.log`, `test-summary-delivery.json`;
formatting/Clippy were repeated after the final tour checkpoint change. Smoke
retains the existing default controller-mapping warning.

PASS release with the existing `skate3` feature (`release-delivery-02.log`).
`launcher-receipt-final.json` binds the successful current tour's executable to
both `target/release/KH2RW.exe` and `target/release/KH2RW Launcher.exe` by exact
hash equality; earlier launcher backups remain intact outside the checkout.

PASS progress generation/check and all nine synthetic progress tests, task-index
regeneration, whitespace and changed-file provenance review. The ledger retains
186 partial native/managed candidates, seven unrun reference scenarios and all
32 final gates; no incomplete inventory is marked complete. New files contain
authored Rust, generated synthetic fixtures and public documentation. Source
bytes, hashes, addresses, pseudocode, captures and private reports remain outside
Git. No dependency, installed-source write, vendor-save write or publication is
introduced. Pattern review (`provenance-review-01.json`) supplements manual
changed-code review; it is not a full security audit.

Complete enemy inventory, source contexts/actor flags, capture services for
Dusk/Soldier, special step paths, secondary HP pools, exact pose blending and
matrix evaluation, effects/audio, native scheduler ordering, complete lock camera
projection/edge response and native HUD layout remain unqualified. Controlled
installed/rewrite comparisons and physical input tests are NOT RUN. Finite
synthetic checks or the authored room do not pass P15/P19/P21/P27 or any final
roadmap gate. No complete functions or 1:1 behavior units are marked PASS.

Verdict: PARTIAL. The three-actor development integration and local delivery
pass their finite checks; full enemy AI/combat and installed comparisons remain
unfinished. Next: extend the remaining source scripts and required native services,
retaining every unsupported unit and installed comparison requirement.
