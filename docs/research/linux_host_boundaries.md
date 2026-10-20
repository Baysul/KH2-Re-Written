# Native Linux host boundary: facts and contract alternatives

## Record and verdict

Wayfinder child: **Establish the native Linux host boundary**. Owner: project_owner;
researcher: delegated leaf. Gate: planning only, not P03/P30 acceptance. Baseline:
`554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f`; branch
`research/linux_host_boundaries`. The first command verified the exact expected
Git common directory, baseline HEAD and branch: **PASS**. Prerequisites read:
worktree instructions, fidelity contract, milestones, unknowns, folder guide,
T170, live map/child/glossary and the historical T171 feasibility report.

**Verdict: viable host seams identified; native feature parity NOT RUN.** This
report neither selects the later human platform contract nor admits any source.
Windows/Steam preservation, native Linux, qualified Steam/Epic inputs and portable
compatible rewrite saves remain the requested scope. No source/save access,
game/application launch, GUI operation, installation, configuration change or test
creation occurred. Only this report was written. Source qualification and
per-session validation remain independent requirements.

## Windowing, graphics and build prerequisites

Both owned manifests explicitly pin Bevy `=0.19.1`, disable defaults and enable
`bevy_winit`/`bevy_gilrs`, but neither Linux window protocol
(`source/main/Cargo.toml:30`, `source/visual/Cargo.toml:20`). The pinned upstream
[Bevy manifest][bevy] defines `x11 -> bevy_internal/x11` and
`wayland -> bevy_internal/wayland`. [Internal dependencies][internal] disable
`bevy_winit` defaults; its [manifest][winit-bevy] forwards to `winit/x11` or
`winit/wayland` plus `winit/wayland-csd-adwaita`. Therefore the intermediate
crate's own default X11 feature does **not** rescue this application. Upstream
[winit 0.30.13][winit] has the explicit Unix compile error when both are absent.
The lock pins winit 0.30.13 and wgpu 29.0.4
(`Cargo.lock:4710-4712`, `Cargo.lock:4345-4347`).

Two viable contract alternatives, **not accepted decisions**:

* **Wayland-only initial Linux target:** explicitly enable `wayland`; smaller
  protocol surface, but no X11 session claim or fallback.
* **Dual backend:** explicitly enable `wayland` and `x11`; qualify Wayland first,
  then separately verify X11/XWayland. Winit's pinned selection code prefers
  Wayland when its environment is present; it does not promise automatic X11
  recovery after a Wayland connection failure. Record the backend actually used.

Keep existing Bevy feature selections and Windows behavior; do not enable all
Bevy defaults just to obtain Linux support. Bevy 0.19.1's declared Rust minimum
is 1.95.0; owned requirements are Rust 1.99 and pinned 1.99.0
(`Cargo.toml:24`, `rust-toolchain.toml:1-5`). Native compilation needs the
`x86_64-unknown-linux-gnu` toolchain/standard library, linker, C/C++ build tools
and pkg-config; preserve the Windows MSVC target. The pinned [Linux dependency
guide][linux-deps] identifies protocol libraries, libudev for gamepads, and ALSA
when using native audio. Availability is necessary, not a successful build.

The [Bevy render manifest][render] selects Vulkan, DX12 and Metal in wgpu, not
GLES. Vulkan loader/driver availability is therefore the concrete Linux starting
point; OpenGL is not a demonstrated fallback for this feature graph. GPU limits,
shader compilation, presentation, scaling, focus and frame pacing still need a
real native verification session. T170's Windows observations are not Linux
renderer evidence (`docs/tasks/T170-lock-camera-and-scan-gauge.md:123-137`).

## Audio: preserve the mixer and Windows device

`Mixer` owns voices, fades, looping and gains; output is 48 kHz interleaved i16
stereo (`source/audio/src/lib.rs:9-10`, `source/audio/src/lib.rs:73-98`,
`source/audio/src/lib.rs:179-244`). `Output` exposes the shared mixer and retains
a Windows-only device; non-Windows currently returns `no audio output on this
platform` (`source/audio/src/lib.rs:247-277`). Preserve `winmm` and its lifetime
semantics rather than rewriting Windows output incidentally.

Candidate **CPAL =0.16.0**, Apache-2.0, provides an adapter-sized option. Its
[pinned manifest][cpal-manifest] uses ALSA 0.9 on Linux, with optional JACK; this
is **not** a direct PipeWire backend. An ALSA default routed through
`pipewire-alsa` is plausible on this host, not verified playback. Its [API
source][cpal-api] provides device/config enumeration, stream callbacks and
explicit start/error handling. Require negotiated sample format, channel count,
rate and buffer size; do not feed a device's arbitrary default rate from the
48 kHz mixer without conversion. Keep the stream alive until output teardown.

Alternative: a Linux-only ALSA adapter (using the same ALSA 0.9 family) with a
bounded worker/buffer ring, reducing abstraction but owning recovery and timing.
A direct PipeWire adapter would require separate pinned API research; it is not
qualified here. Neither choice requires replacing the software mixer with
Bevy's audio stack. Callback integration also needs allocation/locking review:
`mix` currently allocates scratch vectors (`source/audio/src/lib.rs:179-183`)
and the Windows feed locks a mutex (`source/audio/src/lib.rs:259-266`). Audible
output, underruns, device loss, latency and shutdown remain **NOT RUN**.

## Fonts, locations and helper processes

Launcher painting uses fontdue =0.9.4 and Windows font filenames; failure to load
any face aborts painting (`source/main/Cargo.toml:28`,
`source/main/src/launcher/paint.rs:48-60`,
`source/main/src/launcher/paint.rs:103-135`). Two lawful alternatives are a
pinned bundled open font with notices, or explicit host font discovery with a
clear missing-font error. [Liberation Fonts 2.1.5's OFL 1.1][font-license]
permits bundling subject to its notices/license and reserved-name conditions.
It is a candidate, not an approved visual replacement or complete Japanese
coverage solution. Never copy Windows fonts into distribution. Keep launcher
font selection separate from source-derived game font fidelity; measure layout
and missing glyphs rather than assuming metric equivalence.

Current `data_dir()` searches checkout-adjacent `.local/test-room`, then
LOCALAPPDATA, then a working-directory-relative fallback
(`source/main/src/launcher/install.rs:24-39`). Viewer logs require LOCALAPPDATA
(`source/main/src/launcher/install.rs:600-610`). Even `--log` cannot avoid the
smoke's earlier environment lookup (`source/main/src/bin/smoke.rs:19-39`).

Recommended contract candidate, using [XDG Base Directory 0.8][xdg]: application
subdirectories under XDG_CONFIG_HOME for settings; XDG_DATA_HOME for portable
rewrite saves and durable manifests; XDG_CACHE_HOME for regenerable decoded
caches; XDG_STATE_HOME for logs/recent session state. Use the specification's
fallbacks and ignore relative XDG values. Do not place durable saves in
XDG_RUNTIME_DIR. Preserve existing Windows storage with explicit, non-destructive
migration rather than silently changing roots. A consciously selected development
root is an alternative, not an automatic production fallback. Keep caches
source/schema-bound and save compatibility independent of machine paths;
location selection alone does not make the existing save format portable.

Viewer launch appends `.exe` unconditionally and stringifies paths lossily
(`source/main/src/launcher.rs:407-423`); use platform executable suffix and native
path arguments in later implementation. Preserve argument-based `Command`
launching and Windows-only creation flags (`source/main/src/launcher.rs:425-443`).
The Windows-drive browser also needs native roots (`source/main/src/launcher.rs:299-309`).
These are rewrite helpers, not reasons to execute a vendor executable. Optional
skateboard download commands are Windows-specific
(`source/main/src/skate.rs:308-342`); no other game's work is authorized here.

## Bounded observations and remaining evidence

Foreground read-only CLI probes completed with a 25/40-second tool deadline:

| Observation | Result and limit |
|---|---|
| Host/compiler | **PASS inventory:** CachyOS x86_64, Wayland environment; rustc/cargo 1.99.0, native GNU host, GCC 16.2.1, binutils 2.47, pkg-config 3.0.10 |
| Development metadata | **PASS presence:** Wayland client/cursor 1.26.0, xkbcommon and xkbcommon-x11 1.13.2, X11 1.8.14, Xcursor 1.2.3, Xi 1.8.4, Xrandr 1.5.5, ALSA 1.2.16.1, libudev 262, Vulkan 1.4.363 |
| Driver/audio packages | Mesa/vulkan-radeon 26.2.4 and PipeWire/audio/ALSA bridge 1.6.9 installed; no audible or rendering test |
| Hardware | PCI AMD Navi 22 display controller and DRM render node observed; **NOT RUN** actual Vulkan adapter/driver selection, hardware rendering or presentation |
| Fonts/input | Liberation Sans resolves through fontconfig; input event nodes exist. Neither proves application font integration, controller presence, permissions or mappings |

Input already translates Bevy gamepads/keys into the existing controller layer
(`source/visual/src/input.rs:10-145`), but chooses `pads.iter().next()` rather
than establishing a stable device policy. Preserve mappings; later qualify
hotplug/reconnect, stick signs, trigger thresholds, repeat, simultaneous input,
glyph choice and lock-camera chords on the user's selected device. Gilrs-core
0.6.8's [manifest][gilrs] requires Linux libudev; node presence alone is not
permission or controller evidence.

**BLOCKED:** Context7 tool discovery exposed no Context7 resolver/query tools;
no credentials were accessed. Exact-version primary source was fetched instead;
Context7 cross-check remains outstanding. **NOT RUN:** builds/tests (would write
artifacts), GUI/audio/controller probes and all four platform/source comparisons.
The earlier T171 dependency-retrieval block was not re-tested. Next: human review
of backend/audio/font/storage alternatives, then separately authorized build and
isolated hardware-backed verification. No platform contract or fidelity gate is
resolved by this report.

[bevy]: https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/Cargo.toml
[internal]: https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_internal/Cargo.toml
[winit-bevy]: https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_winit/Cargo.toml
[linux-deps]: https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/docs/linux_dependencies.md
[render]: https://github.com/bevyengine/bevy/blob/b56fc29d3016e641754765244b5ba3f9cc504671/crates/bevy_render/Cargo.toml
[winit]: https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/linux/mod.rs
[cpal-manifest]: https://github.com/RustAudio/cpal/blob/v0.16.0/Cargo.toml
[cpal-api]: https://github.com/RustAudio/cpal/blob/v0.16.0/src/lib.rs
[font-license]: https://github.com/liberationfonts/liberation-fonts/blob/2.1.5/LICENSE
[xdg]: https://specifications.freedesktop.org/basedir-spec/0.8/
[gilrs]: https://docs.rs/crate/gilrs-core/0.6.8/source/Cargo.toml.orig
