# Source enemy roster and animation action inspection

The development gallery reads the installed KH2FM object, journal, model,
animation and script records. Source action names and motion candidates are
static inspection evidence. They do not implement the action scheduler, enemy
AI, hitboxes, reactions or encounter rules, and do not qualify P19/P21/P27.

The selected Steam candidate remains quarantined. REA 6.4.0's fresh canonical
managed inspection identifies mixed CLR/native code and retains Evidence
`ev_ad0286c10649ad1fe055bca47488521179e02f0ecf80142e4e721c2cf5aac4f5`.
Native findings below use the separate read-only Ghidra 12.1.4 route; the REA
Windows native provider previously rejected this target class in T120.

Private evidence is under `%LOCALAPPDATA%/KH2FinalRewrite/research/enemy-native-20261013/`:

- `rea-managed-01.json`, `rea-managed-01.stderr.log`, `rea-summary-01.json`:
  fresh execution-free REA identity evidence against the installed executable.
- `native-01/native-audit.json` and `native-02/native-audit.json`, alongside
  private pseudocode and `ghidra-native-01-direct.log`/`ghidra-native-02.log`:
  23 scoped native slices, 1,057 instructions and 3,982 instruction bytes.
  The existing helper checks executable/project/current hash agreement and
  compares each complete reported instruction with its mapped source bytes.
  Source hash is checked again at completion; project edits are discarded.
- `independent-source-bytes-01.json`: a separate file-byte comparison agrees
  for those same instructions. This is not an independent instruction decoder
  and does not qualify complete logical-function boundaries.
- `motion-order-01.json`: independent PE-section mapping and complete 16-byte
  read of the variant preference table. Actor context selection is unqualified.
- `actions-tests-03.log`: six generated-script/header regressions pass. The
  tests contain no extracted scripts, animation payloads or game tables.

The first Java launch failed because its script-path argument was split; its
log is retained as `ghidra-native-01.log`. The corrected direct bootstrap
completed both read-only audits. No installed game was launched or written.

## Roster identity

The pinned [OpenKH journal character layout](https://github.com/OpenKH/OpenKh/blob/7a3b945c538d32c6a285128c98aefba093f52ceb/OpenKh.Kh2/Jiminy/Char.cs)
provides a direct object ID at record offset `+0x0c` and journal display motion
at `+0x0e`. This supports a direct join to installed object entries; the object
entry's gameplay character/status field is a different identity. Journal
categories distinguish Heartless and Nobodies. The pinned
[object entry types](https://github.com/OpenKH/OpenKh/blob/7a3b945c538d32c6a285128c98aefba093f52ceb/OpenKh.Kh2/Objentry.cs)
also include ordinary enemy categories, but an enemy type alone does not prove
Heartless/Nobody membership, story reachability or ordinary-versus-special
semantics. Unresolved candidate records must remain visible in the inventory.

Shared model names can identify the same visual assets across variants. They
do not establish identical AI, stats, animation routing or spawn conditions.
A complete gallery asset inventory is not a complete encounter denominator.

## Named action and motion candidates

`data::enemy_actions::inspect` extends the existing bounded BD-script
decoder with an intentionally narrow compiler-pattern recognizer. It finds
trigger-reachable table-one/action-registration traps whose twelve arguments
are contiguous literal pushes. It retains the action-name string, priority/
flag bits and callback word address. Unsupported/nonliteral registrations are
counted rather than assigned invented names or callbacks.

For each callback it follows direct calls and branches, including resumable
halt paths, with an instruction budget and operand-overlap checks. It records
literal logical-motion candidates for motion-start, motion-push and
frame-start requests. Dynamic requests and callback decoding errors remain
explicit. A branch can contain several motions; static reachability does not
select its live branch, prove it is reachable in gameplay, or determine which
request occurs first. Native PC-specific VM adjustments and indirect callbacks
still require further work.

The retained T106 Dusk script demonstrates why a single universal spawn slot
is unsuitable: its named appearance action branches on appearance mode,
requests a motion and transitions to an idle chosen from actor context; an
alternative appearance path starts an idle at a randomized frame. Attack
actions similarly call other actions and conditional motion helpers. This is
an interpretation of retained script data, not a new installed-game observation.
The broad gallery audit records each currently selected script independently.

## Motion argument and scheduler continuation

[T124](../tasks/T124-enemy-action-arguments-and-scheduler.md) qualifies the
native stack effects and motion trap argument order used by the inspector.
The logical motion cell is separate from dynamic blend and optional start-frame
cells. A bounded backward slice now preserves unchanged integer literals
through supported stack effects above them. It stops at calls, incoming
control-flow joins, unknown effects, gaps or its inspection budget; loaded or
computed motion IDs remain unknown. It does not evaluate the native interpreter's
PC adjustments or choose an action branch.

Fresh Dusk and Shadow script reads match retained inputs after complete selected
source checks. An independent forward stack oracle checks all sixteen emitted
Dusk candidates, including newly exposed attack requests; appearance and idle
retain unresolved helper requests. The complete catalog keeps 8,187 action
records and recovers 407 additional literal requests, for 5,365 total. No previous
candidate is removed; 2,272 actions still contain dynamic requests. Every failed
model and unqualified material profile stays in the finite inspection inventory.

Read-only native research independently matches 34 bounded slices, 1,216
instructions/4,447 bytes and 480 exploratory descriptor rows. The descriptor
sample does not establish complete trap-array membership. The public ledger
records the native entry candidates as PARTIAL, with logical bounds/equivalence
unqualified; none counts as a complete decompiled function.

Normal action push is FIFO. Its sixteen-slot native ring reserves an empty slot,
so only fifteen pending actions fit; the existing Shadow runtime now uses that
limit. Native motion-end is a produced flag polled by the script, not a proven
comparison with a header endpoint. Context, clocks, flag producers, task children,
effects and full enemy behavior still require implementation and comparisons.

## MSET routing

Fresh native lookup evidence establishes that MSET BAR header `+0x0c` bit zero
controls motion indexing. With the bit clear, a logical request uses its direct
slot. With the bit set, the native lookup searches four candidate slots at
`logical_motion * 4 + variant`, preferring a context-dependent order. It checks
a secondary MSET before the primary when present, and resolves the selected
ANB's motion member. The exact preference bytes are retained privately.

`uses_motion_variants` reads the source header after bounded BAR validation.
The gallery can expose every populated candidate of a named action and its
actual raw slot. It must not infer actor context or choose a variant from a
filename. A static preview of a populated slot proves only that the decoder
and renderer can present that retained animation; installed timing, root motion,
effects, texture events, contact windows and transitions need comparisons.

## Raw matrix animation inspection

The first broad source scan exposed type-one raw skeletal animations that the
previous interpolated reader rejected. The new bounded raw reader preserves
the source header, complete bone-matrix and position-matrix tables and extra
span. It uses the pinned OpenKH Motion layout and an explicit discrete stored
matrix inspection profile. Sampling subtracts the declared start frame before
flooring/clamping the matrix index. The declared timeline must span the full
stored frame table with an inclusive or exclusive endpoint; an unknown relationship
remains a failure. The v2 inspection profile holds the last stored pose at the
exclusive endpoint. Nonzero starts
were found in actual source records and are covered by generated regressions.

Private `raw-oracle-02.json` compares 69 retained motions, 209,922 animation
matrices, 794 position matrices and 3,371,456 complete words against an
independent reader, including the complete first and final stored frames.
`raw-tests-final.log`, `raw-clippy-final.log` and `raw-build-final.log` retain
the worker's checks. The skinning path consumes the stored matrices directly;
local quaternion/IK joints remain absent, rather than reconstructed from empty
interpolated curves. Direct local-pose/IK entry points reject raw input.

These finite decoding and matrix-input comparisons do not establish native
interpolation, clock cadence, root rotation/position semantics or screen
equivalence. The gallery's optional root translation and playback remain
inspection choices. Original-only texture inspection likewise does not qualify
HD materials; absent/custom material profiles remain visibly unqualified.

[T123](../tasks/T123-enemy-motion-and-model-boundaries.md) independently checks
27 additional raw records: 14,430 animation matrices, 603 position matrices and
240,528 complete words. Fifteen declare an endpoint one frame after the final
stored matrix. These exact tables and boundary samples qualify the inspection
profile only. The model reader also retains exact EOF links under the pinned
legacy reader's empty-substream termination rule; partial/outside links fail.

## Remaining requirements

Complete enemy behavior needs the full per-script action/event graph, native
trap semantics and PC adjustments, scheduler/priority/queue behavior, target
selection, actor flags, root/collision movement, attack definitions and damage,
reaction/death/prize behavior, effects/audio, encounter activation and measured
installed/rewrite scenarios. Unsupported raw-motion formats or omitted material
systems must be reported as failures or unknowns. Finite source parsing and a
debug gallery do not satisfy the requested complete enemy implementation.

## Executable ordinary enemy combat

[T125](../tasks/T125-enemy-ai-and-combat.md) connects ordinary Shadow, Dusk and
Soldier to source-driven action state machines and shared Sora combat in the
generated arena. F3 actor controls enable AI, freeze, defeat and respawn. Other
catalog entries still expose source animation inspection and explicit unsupported
AI status. This does not complete the 304-candidate inventory.

Source TARGET shapes carry stable actor/shape identity through lock-on and
cycling; the selected shape drives attacks and spells. Equipped Scan gates a
plain live-HP debug panel. The lock camera uses the admitted source anchor,
look weighting, radius and spring subset; projection/edge response, native HUD
layout and controlled installed comparisons remain unqualified.
