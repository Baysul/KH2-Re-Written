# Audio output

`src/lib.rs` mixes sounds. `src/winmm.rs` adapts output to Windows and
`src/linux_device.rs` to Linux (CPAL's ALSA host, usually PipeWire's ALSA
bridge). Sound-file readers live in `data/`; scene sound requests live in
`visual/`.

See the [folder guide](../../docs/folder-guide.md) and [architecture](../../ARCHITECTURE.md).
