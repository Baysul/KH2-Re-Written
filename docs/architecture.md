# Architecture at foundation

Three implemented crates: `source_identity` reads/hash-identifies the selected installed source without gameplay assumptions; `package_reader` validates bounded index and header structures without decoding payloads; `windows_shell` is a source-independent Bevy development smoke app. It does not run KH2 and its rectangle is not player logic.

Future boundaries from the roadmap: bounded package readers -> traceable content representation -> transactional source-bound cache; authoritative headless game state -> explicit script services -> render/audio views. Input and Windows services are adapters. Saves serialize versioned game state. Renderer/window/input polling cannot award progression. Interfaces will carry immutable source identity. Add a crate only when a tested behavior needs the boundary.

The first source manifest is intentionally an incomplete candidate inventory. Any source-independent smoke fixture is generated in this repository and has no connection to installed assets. No decoder, script VM, combat rule or guessed simulation loop has been implemented.
