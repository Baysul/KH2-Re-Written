# T179 — Linux sound output behind the existing mixer

Owner: coding agent. Gate: P03/P26 native-platform development evidence only; no audio fidelity, source, gameplay or compatibility gate advances. Prerequisites: T177, T178, the approved [native Linux platform decision](T175_native_linux_platform.md) (sound: keep the mixer and Windows waveOut, add a Linux-only adapter; CPAL 0.16.0 as the researched starting point) and the [first-port acceptance checklist](T176_port_acceptance.md). The user asked to continue the implementation.

## Scope and identity

Give `audio::Output` a Linux device so the existing mixer plays on the Linux host. The mixer, its 48 kHz interleaved 16-bit stereo, the `Output` API and the Windows waveOut module stay unchanged. No test is added or changed. No game data is used.

Baseline `554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f` (branch `epic-linux`) plus the uncommitted T177/T178 changes; Rust/Cargo 1.99.0. New Linux-only dependency `cpal = "=0.16.0"` with default features off. The checksum of the downloaded crate archive (`cbd307f4…`) matches the lock, and its licence is Apache-2.0. Host: the x86_64 CachyOS/Wayland development machine. Its sound system is reached through ALSA's default device, the PipeWire bridge.

## Evidence locations and method

Private evidence bundles **T179-B01** (logs, receipts, probe results, lock copies) and **T179-C01** (the downloaded crate archive and extraction used to read the pinned API), outside the checkout. The pinned crate source, not general documentation, is the API authority: `build_output_stream`, `supported_output_configs`, `SampleRate`, `StreamError` and the ALSA host's underrun handling were read there. Bounded runs used `jev-fabric exec` with deadlines; long checks ran as supervised jobs.

Cleanup note: T194 removes the temporary `output_check` example. The probe
commands and results below describe the historical T179 runs; they are not
current build instructions.

## Steps and observations

1. Read the mixer and waveOut device: a dedicated thread pulls fixed buffers from the shared mixer behind a mutex, and `Output` owns the device and stops it on drop. `Output` lives inside Bevy resources, so it must stay `Send + Sync`.
2. Read the pinned CPAL source. No default features; ALSA is a plain Linux dependency, and JACK and thread priority stay off. The output callback receives whole interleaved frames. The ALSA host **recovers from underruns without telling the caller** (`TODO: Notify the user`), so underruns cannot be counted through CPAL.
3. Add `source/audio/src/linux_device.rs`, mirroring `winmm::Device`. A named `audio` thread opens the default output device and keeps the stream until the device is dropped, so `Device` is `Send + Sync` whatever CPAL's stream type is. The opening result is handed back to `Output::open`, so errors reach the caller. The device chooses only a **stereo configuration at exactly the mixer's 48 kHz**, 16-bit first, then float: there is no silent resampling, and a device without one is an error. Float output scales the mixer's 16-bit samples into a scratch buffer that only grows. Stream errors are written to standard error; the opened device, format and rate are reported once on standard error.
4. `Output::open` gains a Linux branch with the same feed closure as Windows; the Windows branch is untouched. Other platforms still report "no audio output on this platform".
5. Lock review: an editor (rust-analyzer in the user's open Zed window on this worktree) re-resolved the lock as soon as the manifest changed, before Cargo was run here. Against the git baseline, all 444 original entries are unchanged and 23 CPAL entries are added on top of T177's 45. Most of the 23 are other-platform CPAL dependencies that Cargo records but never builds here. The same editor runs its own checks in this `target/` folder, which most likely caused T178's one-off doc-test failure; it was left alone.
6. Add the developer probe `audio` example `output_check`. It opens `Output`, waits for the first pulled buffer, counts the stereo frames the device pulls over a set time, and reports the rate. It is silent unless `--tone` adds a quiet 440 Hz sine for a person to listen to. It is a maintained check like the smoke, not a test.
7. Run the silent probe against the real session twice, 5 s each, then once with the sound server unreachable (results below). Silent output was used on purpose, so nothing played through the user's speakers.
8. Workspace checks. The first run, in the shared `target/` folder, passed formatting and clippy, then failed one doc-test (`visual`: can't find crate for `bevy`) after the editor rebuilt `bevy` mid-run; all 725 unit tests passed. A rerun in a private build folder failed to compile `data`, because T180's validation edits were saved while it was building (a mixed tree, not a T179 fault). The checks were then run once on the combined T179+T180 tree in a private build folder and all passed (recorded in [T180](T180_epic_source_binding.md), bundle T180-B01).

## Checks

| Check | Result |
| --- | --- |
| Silent probe, real session (two runs) | PASS: `default` device, I16, 48000 Hz, 2 channels; 240745 frames in 5.000 s each run (48148 frames/s, one buffer of read-ahead over 240000); first buffer after 6–8 ms; no stream errors |
| Sound server unreachable | PASS (expected failure): `snd_pcm_open … Host is down (112)`, exit 1, no hang |
| `cargo clippy -p audio --all-targets` (Linux), warnings denied | PASS |
| Windows-target clippy of `audio`, all targets, warnings denied | PASS (compile-only cross-check) |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS (combined T179+T180 tree, private build folder) |
| `cargo test --workspace --locked` (existing mixer tests included) | PASS on the combined tree: 725 passed, 0 failed, 1 ignored, 28 test programs. Earlier attempts: FAIL (shared-folder doc-test, editor rebuild) and FAIL (mixed tree during T180 edits), see step 8 |
| `cargo build --workspace --locked` | PASS (combined tree) |
| Audible playback, check tone through speakers or headphones | NOT RUN for the tone probe (`cargo run -p audio --example output_check --locked -- 3 --tone`); audible game sound is covered by the next rows |
| Underruns, device unplug/replug, latency, long-run stability | NOT RUN as measurements: CPAL does not report ALSA underruns; no device-change or latency measurement. The user heard no crackling in a short session (T180 step 10) |
| Game sound (title, test room) on Linux | PASS (user-reported): in the user's own session, from the Epic copy, the title and the test room played sound with no crackling (T180 step 10) |
| Game sound in the event player on Linux | NOT RUN: not listened to |
| Native Windows sound regression | NOT RUN: no Windows host; Windows code unchanged |
| Audio fidelity (onset, loop, mix, transitions) | NOT RUN: P26 comparisons are separate |

## Uncertainty, verdict and next work

**PASS** for this slice: the Linux device opens, negotiates 48 kHz stereo and is pulled at the mixer's rate; the workspace checks pass. The user later heard the title and test room play without crackling (T180 step 10). Measured underruns, device changes, latency and the event player's sound remain NOT RUN.

Limits: the probe proves negotiation, stream lifetime and pull rate at the ALSA client, not sound at the speakers. The mixer still allocates scratch buffers on each mix and the feed takes the mixer's mutex, the same as the Windows feed does. Underruns caused by that cannot be seen through CPAL. If listening or later gameplay shows dropouts, measure first, then consider a preallocated mixer buffer (a shared-code change). A device whose only configurations are not 48 kHz stereo is refused with a clear message; adding a verified resampler would be a separate decision. No regression test was added. A useful one would feed `stereo_config`'s selection rule synthetic configuration lists (match, wrong rate, mono only, float only) and needs approval first.

Next: the Epic source profile and the warned no-save title start ([T180](T180_epic_source_binding.md)). That unlocks Linux checks of the title, test room, viewers and game sound.
