# Bounded static native research

The selected Steam executable contains native code and CLR metadata. T010's ILSpy path remains the metadata authority. Native research uses a separately qualified direct Ghidra engine; this does not route a mixed image through REA's native-only Windows P0 admission or qualify gameplay.

Pinned engine: [Ghidra 12.1.4 official release](https://github.com/NationalSecurityAgency/ghidra/releases/tag/Ghidra_12.1.4_build), revision `8b6bbb857accdfa20dc5b2f5dea471178c2e9fbc`, distribution `ghidra_12.1.4_PUBLIC_20260921.zip`. Published SHA-256: `ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db`; the downloaded archive matches. The Apache-2.0 root license, bundled licenses and SBOM remain with the external tool. A full transitive distribution audit is unfinished. No Ghidra code or binary is linked into the Rust product.

The [pinned application properties](https://github.com/NationalSecurityAgency/ghidra/blob/Ghidra_12.1.4_build/Ghidra/application.properties) require Java 21 or later with no configured maximum. Actual qualification used full JDK 26.0.2.1 x64, MSVC 14.29 and the shipped Windows native decompiler. This is tested host compatibility, not a general promise for other versions. No JDK update or paid tool is required.

Put the archive in `<private-tool-directory>/ghidra_12.1.4_PUBLIC_20260921.zip`; extract it into `<private-tool-directory>/distribution/ghidra_12.1.4_PUBLIC/`. Each launcher invocation verifies the archive digest and eight core installed components against their archive entries, including the decompiler executable, x86 language and Windows compiler specification. Other bundled files and host libraries are not independently attested by that core comparison.

Run the authored oracle first, using an existing empty private directory under `%LOCALAPPDATA%/KH2FinalRewrite/research/`:

```powershell
pwsh -NoProfile -File tools/Verify-NativeReader.ps1 `
  -ToolDirectory '<private-tool-directory>' -JavaHome '<full-jdk-root>' `
  -CompilerDirectory '<msvc-host-x64-target-x64-bin-directory>' `
  -InstallRoot '<selected-steam-install-root>' `
  -OutputDirectory '<new-private-conformance-directory>'
```

The two-method fixture is compiled without CRT imports/entry point, statically decompiled, then recompiled. Recompilation supplies the authored fixture's known global initializer and scalar typedefs explicitly. Twenty-two finite comparisons check the original binary, recovered binary and authored expected outputs, including unsigned boundaries. Only authored binaries are loaded/executed. Neither the fixture nor these finite cases represents KH2 rules or whole-decompiler correctness.

Native selection JSON binds `executable_sha256` and selects one to eight `methods`, each with `rva`, `file_offset`, `prefix_size` (1–64) and `prefix_sha256`. Steam input additionally requires the managed inventory's `source_content_identity` and the exact native MethodDef `token` on each row. All values come from private source-hashed observations. Prefix length verifies an entry span, not function length. Use the current oracle and T010 inventory:

```powershell
pwsh -NoProfile -File tools/Invoke-NativeReader.ps1 `
  -ToolDirectory '<private-tool-directory>' -JavaHome '<full-jdk-root>' `
  -InstallRoot '<selected-steam-install-root>' `
  -InputFile '<selected-steam-install-root>/KINGDOM HEARTS II FINAL MIX.exe' `
  -SelectionPath '<private-native-selection.json>' `
  -ManagedInventory '<private-managed-inspection>/inventory.json' `
  -ConformanceReceipt '<private-conformance-directory>/conformance.json' `
  -OutputDirectory '<new-private-native-output-directory>'
```

Steam input is restricted to this executable, bound native MethodDefs or source-proven direct branches, and the current authored-fixture/tool/host receipt. Private authored fixtures can use the generic reader path without that prior receipt, to bootstrap the oracle. This developer path validates the selected executable and links the earlier candidate commitment; it does not freshly scan all packages or label any build supported.

To follow an instruction recorded in a prior qualified native run, pass `-ParentNativeDirectory '<private-parent-run>'` and a selection row with `kind: direct_branch`, `parent_report_sha256`, `parent_function_rva` and `instruction_rva`, plus the ordinary target `rva`, `file_offset`, prefix size/hash. The parent receipt/report must match current input, native reader, engine/core components, Java, import profile and PE mappings. The caller must occur uniquely in the selected parent body's listing. The launcher checks its complete five-byte span against the current physical source and admits only `E8`/`E9` rel32 CALL/JMP; it computes the target from the signed displacement and validates the target's PE mapping. These encodings/relative semantics agree with the [pinned x86 language source](https://github.com/NationalSecurityAgency/ghidra/blob/8b6bbb857accdfa20dc5b2f5dea471178c2e9fbc/Ghidra/Processors/x86/data/languages/ia.sinc) and independent installed-source disassembly. It does not choose an address from pseudocode text. Indirect calls, imports, unresolved targets and other encodings remain unqualified by this route. New receipts retain the parent report hash and branch-helper hash.

For a bounded static indirect observation, use `kind: static_pointer` with the same parent fields and target prefix, plus `slot_rva`, `slot_file_offset` and `slot_sha256`. Only unprefixed six-byte `FF 15`/`FF 25` RIP-relative CALL/JMP instructions are admitted. The launcher validates the complete recorded instruction, decodes the signed slot displacement, checks all eight slot bytes against the selection and subtracts the independently parsed preferred image base from the file pointer. The candidate must map to executable bytes in the native reader. A receipt counts these leads explicitly; **this observes a file pointer, not a live destination**. Imports, register-indirect calls, other encodings, out-of-image values and runtime rebinding remain unresolved. The encoding/relative semantics are checked against the same pinned language source and independent installed disassembly.

Run `pwsh -NoProfile -File tools/Verify-NativeBranches.ps1` for 23 generated synthetic direct/indirect branch and mapping checks. T014 additionally exercised the selected-source admission path with generated authored fixture bytes: five invalid parent/target cases rejected before engine launch and one valid case reached the bounded reader. T015 adds four invalid indirect slot/target cases and a positive generated admission fixture. Mocked parent/inventory records are explicit test objects; these tests do not fabricate source support or actual game metadata.

The launcher uses the official Java bootstrap with private settings/cache/temp directories, `PeLoader`, `x86:LE:64:default`, Windows compiler specification, CLR parsing disabled, external library loading/linking disabled, ordinal lookup disabled and no auto-analysis. T013 found that the loader's default ordinal lookup still read external library metadata despite disabling library imports. The [pinned ordinal-support loader](https://github.com/NationalSecurityAgency/ghidra/blob/8b6bbb857accdfa20dc5b2f5dea471178c2e9fbc/Ghidra/Features/Base/src/main/java/ghidra/app/util/opinion/AbstractOrdinalSupportLoader.java) supplies the separate `-loader-ordinalLookup false` option. Receipts identify `native_pe_no_cli_no_ordinal_lookup_v1`; Steam research rejects an oracle with a different/missing profile or stale reader hash. Run a new authored oracle after reader/profile changes.

Its default read-only import saves no project. Optional `-PreservePrivateProject` retains `projects/SelectedNative.gpr` and its `.rep` database under the same guarded private output directory; the receipt records this choice and requires both outputs. This database contains proprietary derived bytes and must remain outside Git, backups of repository code and distribution. The installed input stays read-only in both modes.

The imported base must match independently parsed PE headers. No target is executed. Prefix and instruction bytes are checked against the source snapshot and physical PE offsets; findings are bounded to 4,096 body bytes, 256 instructions and 30 seconds of decompilation per selection. Imported parsing/disassembly time is not a strict total wall-clock deadline. A function estimate is not proof of all call paths, calling convention or live behavior.

Loader-declared functions can lack instructions or have a one-byte placeholder body. The reader now disassembles a missing entry, recomputes the declared body through the pinned engine's `CreateFunctionCmd.fixupFunctionBody`, and rejects any instruction that extends outside it. Limits are applied after reconstruction. The authored declared-entry regression reproduces the old failure and checks the fixed body; the current 22-case behavior oracle also passes. Decompiler output can incorporate tail-call flow beyond the independently checked listing; it is a lead, not proof that every displayed operation has been verified. T013 independently compares two startup-related listings and rechecks both earlier getters.

The report contains proprietary pseudocode/disassembly and must remain private. Headless script failures can return process exit zero; the wrapper also requires the bound report before producing a PASS receipt. Failed runs can leave private logs/caches or partial outputs; preserve them and retry in a fresh directory. See [T011](tasks/T011-native-reader.md) for qualification, rejected initial CLR import, independent instruction checks and uncertainty.

For the separately qualified pinned MCP bridge, prepared-project routing and explicit close procedure, see [bridge inspection](bridge-inspection.md) and [T012](tasks/T012-bridge-qualification.md). Engine qualification alone does not establish bridge compatibility.

See [T013](tasks/T013-startup-path.md) for the PE/CLR entry distinction, loader isolation, body reconstruction and finite independent startup evidence. Actual startup ordering, game initialization and active callers remain unqualified.

[T014](tasks/T014-startup-followup.md) follows two proven direct branches beyond the CLR MethodDef list and independently checks the startup worker and its call target. A mutable indirect-call slot is retained as a static lead; its file value does not establish a live destination.

[T015](tasks/T015-static-indirect-entry.md) qualifies that static-slot route and independently checks its application entry candidate. Recorded global operands/direct branches guide subsequent initialization research without establishing ownership or live execution.
