# T106 — Twilight Town enemy inventory and Rust script decoding

Owner: Codex. Date: 2026-10-11. Gates: P07/P10/P12 research supporting P19/P21/P27; no gameplay gate advancement.

## Request and prerequisites

User requests Ghidra-backed Rust implementation of Twilight Town's ordinary Heartless and Nobodies across all visits, explicitly excluding Mushroom XIII and special variants. **That gameplay request remains unfinished.** This record covers the verified inventory/decoder work, not completed enemies.

Read AGENTS, fidelity contract, milestones, unknowns, latest task T103, current-state, and relevant T066/T080/T097 records. Preserve the pre-existing T102 documentation and task-index edits. Existing unrelated optional functionality is untouched. The current runtime is coupled to a single Shadow, and T097 does not implement area battles.

## Source and build identity

Checkout base: `43685b37e4e43ebebacec18469198b66875d014d`. Selected KH2FM Steam candidate reports build 15194255. Both Rust inventory runs freshly hash the complete thirteen-file candidate before opening packages; research admission retains quarantine. Private reports bind the exact content identity. Ghidra 12.1.4 opens the existing a2 project read-only, with analysis disabled; the research script requires its executable SHA-256 to match the freshly hashed installed KH2 executable before decompilation. No gameplay session or vendor save is opened.

Private evidence root: `research/tt-enemies-20261011/` within the Codex app's local KH2FinalRewrite storage. Package-isolated storage is required when accessing it from the separate Python process; no personal absolute paths are recorded here.

## Steps and exact evidence

1. `roster-01.json`: source-bound Rust survey of every type-12 set in all 42 English Twilight Town ARDs. Records 2,043 placements and 151 distinct object IDs, retaining area, BAR entry, point, entity, spawn arguments and resolved object-entry fields. These are stored records across programs, **not proof that every record is reachable** at a particular story state.
2. `EnemyEngine.java`, `ghidra-01.log`, `identity.txt`: Ghidra source-hash admission and four fresh decompilations, saved as `14041b400.c`, `14041c900.c`, `1403e1410.c`, `1403ca550.c`. The native interpreter includes PC-specific immediate/value adjustments; decoding the original scripts alone does not reproduce those semantics.
3. Added `package_reader::bdscript`: bounded header/trigger parsing, raw instruction operands, signed direct branch/call targets, and trigger-rooted control-flow traversal. Retains float bits; rejects truncated instructions, unsupported opcodes, invalid targets and operand overlap. Halt has a resume edge; exit/return terminate a path. Does not guess indirect action callbacks or execute any trap.
4. `roster-02.json`: the same placements plus 33 model AI scripts selected through object-entry types 4/21. Includes 32,200 direct-graph instructions, with explicit error/coverage fields. The broad inspection denominator intentionally retains out-of-scope opponents and props; type 4 alone does not mean ordinary Heartless/Nobody.
5. `oracle.py`, `oracle-01.json`: separate Python package/BAR/ARD reading matches every reported placement; the previous private OpenKH-based opcode descriptor decoder independently matches all reported script headers, triggers and direct-graph instructions. Per-script SHA-256 identities and private BDX snapshots are retained. OpenKH local checkout pin is `7a3b945c538d32c6a285128c98aefba093f52ceb`; checked its actual header/opcode sources and updated NOTICE.
6. `dusk-listing-01.txt`: private exploratory Dusk listing identifies action registration beyond the direct trigger graph. It is not accepted as a complete executable behavior translation. Existing private Shadow/engine research remains unchanged.

## Coverage and uncertainty

The survey contains 21 unsuffixed `M_EXnnn` model candidates. A filename pattern is not semantic roster qualification: resolve journal identity and encounter-program reachability before claiming a complete ordinary-enemy roster. Explicitly exclude Mushroom XIII, bosses, struggle opponents and special variants from implementation scope while retaining them in the private survey.

The 33 direct graphs reference 148 distinct trap pairs. Indirect act entry points, trap implementations, scheduler/PC adjustments, motion/attack bindings, reactions, encounter activation and multiple-enemy target/hit identity remain to implement and compare. No new enemy AI or area battle has been enabled by this change. Four synthetic tests validate parsing/control-flow boundaries, not enemy behavior. Finite script decoding cannot establish whole-game equivalence.

Ghidra admission compares the project's recorded executable hash, not every mapped project byte against the raw executable; fresh independent native instruction comparison is NOT RUN. The independent Python comparison validates script data decoding only. The reused Ghidra project is a research aid, not a new executable qualification.

## Checks

- Complete source validation for both probe runs: PASS.
- Ghidra executable-hash admission and four fresh decompilations: PASS.
- Independent 2,043-placement and 33-script/32,200-instruction comparison: PASS.
- Four authored synthetic decoder tests: PASS.
- `cargo fmt --all --check`: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- `cargo test --workspace --locked`: PASS.
- `cargo build --workspace --locked`: PASS.
- `cargo run -p windows_shell --locked -- --frames 180`: PASS (exit 0; existing gamepad default-mapping warning).
- Diff whitespace and provenance review of the added Rust/source record: PASS; no assets, extracted instructions, captures, saves or personal paths added to Git content. Private evidence stays external; no push/publication.
- Installed combat/reference comparisons and new-enemy gameplay: NOT RUN.

## Verdict

PARTIAL. Source-bound inventory and direct script decoding pass their finite checks. The requested ordinary-enemy implementation remains incomplete; P19/P21/P27 do not pass.

## Next task

Resolve the ordinary roster against journal/encounter identities, qualify indirect act registration and required native traps, then implement the first complete non-Shadow enemy with multiple-actor targeting/hits and source-bound Twilight Town encounter activation. Continue through every ordinary enemy across visits, preserving explicit failures and reference-comparison gaps.
