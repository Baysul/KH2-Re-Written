# P03 measured development environment

Measured 2026-10-03 on this machine. Private local logs: `%USERPROFILE%/KH2FinalRewrite-private/logs/evidence-2026-10-03/smoke-*.{log,stdout.log,stderr.log}`. Source hashes and package records are in external `%LOCALAPPDATA%/KH2FinalRewrite/research/foundation-20261003/`.

| Item | Observation |
|---|---|
| Windows | Windows 11 Home, 10.0.26200, 64-bit |
| CPU | Intel Core i7-9700F @ 3.00 GHz |
| GPU / driver | NVIDIA GeForce RTX 2070 SUPER / 32.0.16.1692 |
| RAM | 34,298,855,424 bytes |
| Free C: space at start | 193,343,246,336 bytes |
| Rust / Cargo | 1.99.0 / 1.99.0; pinned stable toolchain |
| Target | x86_64-pc-windows-msvc |
| C++ tools | Visual Studio Community 2019 16.11.56, MSVC 14.29.30133 |
| Windows SDK | 10.0.19041.0 |
| Bevy | Stable 0.19.1, MSRV 1.95; selected after registry/upstream inspection |
| Link / renderer | Native window and synthetic sprite successfully launch; exact graphics backend/driver qualification pending |
| Devices | Bevy enumerates two gamepads; unrecognized mapping warning, idle axes/calibration require review |
| .NET | Runtime executable exists; no SDK reported; OpenKH .NET 8 tools not executed |
| Display | Smoke window 960x540; reference resolution/refresh/HDR/controller baseline unconfirmed |

P03 automated smoke: window startup, imported generated BMP loaded, 180 frames, clean exit code 0. Interactive rendering and Escape observed through Windows Computer Use. Unrecognized gamepad idle motion led to explicit `--gamepad` opt-in; keyboard mode is stable independently of that calibration. Physical gamepad button/axis behavior and limited-permission/second-machine tests remain NOT RUN.

Reference launch is separate. Direct installed KH2FM launch produced no usable window; retry with the installation as working directory exited -1073741819 (0xC0000005). No cause is asserted. The user then launched normally through Steam; credits and KH2FM title were observed (T002), with private save/config copy and screenshots. Injected keyboard/click inputs have not yielded a verified settings transition; physical assistance requested. No edition swap, Steam repair or bypass was performed. The known pinned OpenKH transform was independently executed in memory via .NET Add-Type (T005), without installing an SDK or invoking a source-writing tool.

Build/check commands are in README and AGENTS. CI is authored for Windows but has not been dispatched. This machine is development evidence, not a claimed release minimum. Tool paths and external manifests remain local; an independent checkout must resolve only pinned crates and the generated test asset.
