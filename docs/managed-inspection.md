# Static managed research

These developer tools inspect the selected KH2FM executable as bytes. They do not launch it, import it as a .NET assembly, qualify a supported build or enable gameplay. Native implementations need a separately qualified native reader. Findings and pseudocode remain private.

The external research dependency is MIT-licensed `ICSharpCode.Decompiler` **9.1.0.7988**, selected from [ILSpy v9.1](https://github.com/icsharpcode/ILSpy/releases/tag/v9.1), release commit `03b7444943e720b3134d296c0c8dd3876f8ea4ce`. The [official NuGet package](https://www.nuget.org/packages/ICSharpCode.Decompiler/9.1.0.7988) supplies a netstandard2.0 library. Its package SHA-256 is `ccf2e02cd3b8702aed37d051f71e97f96d0c36443bd67003228a4e0b55418404`; the library SHA-256 is `2e47c9b7557d071935f4730d5cd692162bd0f6266ce7d29f1d13ffeb6b2dae75`. HTTPS origin and local hash pinning were checked; the package signature was not independently verified. No SDK upgrade, GUI installation or Cargo dependency is required for this research path.

Place the package at `<private-tool-directory>/icsharpcode.decompiler.9.1.0.7988.nupkg` and its contents under `<private-tool-directory>/package/`. Our initializer verifies the package hash and compares the installed DLL with the DLL inside that package before loading the tool. Actual host qualification used PowerShell's .NET 10.0.11 runtime and already-loaded metadata/immutable collection assemblies 10.0.0.0; another host must pass the oracle rather than assume compatibility from those versions.

All output directories must already exist under `%LOCALAPPDATA%/KH2FinalRewrite/research/`, outside Git and the selected installation. Create fresh directories for every run. File creation refuses overwrite. Windows packaged-app storage can redirect the logical local-app-data path: handle-based canonical paths bind both aliases to the same physical location. Reparse ancestors and UNC destinations are unqualified. The resolver uses a private base path and host/private references; missing references stay unresolved rather than admitting Steam sibling assemblies.

First qualify the reader using only the authored fixture:

```powershell
pwsh -NoProfile -File tools/Verify-ManagedReader.ps1 `
  -ToolDirectory '<private-tool-directory>' `
  -InstallRoot '<selected-install-root>' `
  -OutputDirectory '<new-private-conformance-directory>'
```

This compiles our two-method fixture, statically decompiles it, recompiles under another namespace and compares 22 cases, including integer boundaries and switch defaults. Only authored assemblies are executed. The receipt binds the tool hashes and current fixture source hash. Finite cases do not establish general decompiler correctness.

Use a candidate manifest from `source_identity`. A private selection JSON must contain `schema_version: 1`, the manifest's `source_content_identity`, the bound `executable_sha256`, and `method_tokens`: one to eight unique lowercase MethodDef tokens such as `0x06000001`. Choose tokens from an observed metadata inventory, not guessed behavior. Then run:

```powershell
pwsh -NoProfile -File tools/Inspect-ManagedExecutable.ps1 `
  -InstallRoot '<selected-install-root>' `
  -ManifestPath '<private-candidate-manifest>' `
  -ToolDirectory '<private-tool-directory>' `
  -ConformanceReceipt '<private-conformance-directory>/conformance.json' `
  -SelectionPath '<private-selection.json>' `
  -OutputDirectory '<new-private-inspection-directory>'
```

The tool validates the 13-file candidate commitment, canonical source path and receipt/selection, then hashes a bounded executable snapshot and rehashes the installed executable after analysis. **Only this executable is freshly verified by this command**; a candidate commitment is not a full contemporaneous package scan or qualified dependency closure. The snapshot is never persisted. The private inventory reports MethodDefs, native/IL markers and physical PE file offsets. CIL spans are independently mapped through tiny/fat headers and compared with metadata-reader hashes. Up to eight selected methods receive bounded pseudocode; native/bodyless entries receive declarations only. Pseudocode is analysis output, may contain unresolved references or C++/CLI constructs, and is not original compilable source or measured behavior.

An I/O failure can leave partial new output files; preserve the failed directory and retry in a fresh one. Do not place receipts, inventories, executable hashes, pseudocode or snapshots in Git or distribution. See [T010](tasks/T010-managed-reader.md) for actual installed-build observations and checks.
