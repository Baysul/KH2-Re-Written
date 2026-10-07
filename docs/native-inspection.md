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

Steam input is restricted to this executable, bound native MethodDefs and the current authored-fixture/tool/host receipt. Private authored fixtures can use the generic reader path without that prior receipt, to bootstrap the oracle. This developer path validates the selected executable and links the earlier candidate commitment; it does not freshly scan all packages or label any build supported.

The launcher uses the official Java bootstrap with private settings/cache/temp directories, `PeLoader`, `x86:LE:64:default`, Windows compiler specification, CLR parsing disabled, external library loading/linking disabled, no auto-analysis and read-only import. The imported base must match independently parsed PE headers. No target is executed and no program is saved to a project. Prefix and instruction bytes are checked against the source snapshot and physical PE offsets; findings are bounded to 4,096 body bytes, 256 instructions and 30 seconds of decompilation per selection. Imported parsing/disassembly time is not a strict total wall-clock deadline. A function estimate is not proof of all call paths, calling convention or live behavior.

The report contains proprietary pseudocode/disassembly and must remain private. Headless script failures can return process exit zero; the wrapper also requires the bound report before producing a PASS receipt. Failed runs can leave private logs/caches or partial outputs; preserve them and retry in a fresh directory. See [T011](tasks/T011-native-reader.md) for qualification, rejected initial CLR import, independent instruction checks and uncertainty.
