# Roadmap status and work queue

Updated 2026-10-03. The [user-supplied roadmap](https://docs.google.com/document/d/1lewKD-h2ZVOOES1qugUgj82QVMybWwb8sqK3ed-4wPI/edit?tab=t.0) is the acceptance authority. Core scope is confirmed: KH2FM only, installed Steam source. No final gate is represented as passed where user review, reference evidence or dependencies are missing. Source inspection/build/records below are the expressly allowed fallback while P00 is draft.

| Gate | Objective | Status / evidence / dependency |
|---|---|---|
| P00 | Scope and fidelity contract | Draft authored; core scope confirmed; display/controller/tolerances, vendor-save and voice decisions pending |
| P01 | Installed-source and release boundary | Policy authored; candidate scanner source/output isolation tested; launch/cache/release enforcement pending |
| P02 | Installed edition/build identity | 13 files / 16.4 GB hashed twice identically; edition/integrity/shared/language closure unqualified |
| P03 | Windows/Rust/Bevy environment | Build, image load, native window, clean timed exit and interactive Escape passed; reference boot/controller/second environment pending |
| P04 | Durable repository/evidence | Instructions, records, schemas, CI and provenance authored; clean-checkout/backup recovery verification in foundation task |
| P05 | Pinned tool/license audit | OpenKH/Skate3/iw4L pinned and inspected; six HEDs independently checked; payload extraction fixture and tool license gaps remain |
| P06 | Native folder picker and source validation | Not started; supported edition/file profile P02 must be qualified; do not enable play on filename evidence |
| P07 | Packages/archives/dependencies | Preliminary index/header inspection: 46,076 records; all checked ranges valid; payload/name/language/replacement closure pending |
| P08 | Read-only importer/source-bound cache | Not started; P06/P07 qualified input required |
| P09 | Reference capture/replay harness | Not started; installed reference title/session and backed-up save/config required |
| P10 | Installed executable/runtime research | Not started; qualified binary and controlled reference observations required |
| P11 | Authoritative simulation boundary | Not started; measured time/input/state contract required |
| P12 | Traceable decoded representation | Not started; independently checked reader corpus required |
| P13 | Asset/animation inspection app | Not started; actual decoded assets required |
| P14 | PC rendering/presentation | Not started; P09/P12/P13 reference alignment required |
| P15 | Animation/motion events | Not started; decoded tracks and observed triggers required |
| P16 | Rooms/collision/streaming/actors | Not started; traceable room data and scenarios required |
| P17 | Input/time/RNG/determinism | Not started; actual clock/input/RNG evidence required |
| P18 | Movement/camera/player states | Not started; measured reference rules and P16/P17 required |
| P19 | Combat/enemies/reactions | Not started; observed state rules required |
| P20 | Scripts/native services | Not started; decoded scripts and traced services required |
| P21 | Complete playable slice | Not started; movement/fight/script/save dependencies must be demonstrated |
| P22 | Progression/party/inventory/systems | Not started; authoritative rules/data required |
| P23 | Native saves/safe vendor import | Not started; measured state/schema and import scope required |
| P24 | Story/cutscenes/transitions | Not started; script/event/audio/streaming evidence required |
| P25 | Menus/HUD/settings/EN/JA/fonts | Not started; language sources and paired behavior verification required |
| P26 | Audio/music/synchronization | Not started; decoded sources and measured scheduling required |
| P27 | Every world/encounter/mode | Not started; complete discovered denominator and implementation required |
| P28 | Campaign and both text options | Not started; full P27 route ledger and user play/review required |
| P29 | Steam changes/recovery | Not started; complete profile/cache/save implementations required |
| P30 | Performance/Windows reliability | Not started; full candidate and measured hardware matrix required |
| P31 | Windows package/release qualification | Not started; all required gates and separate user release decision required |

Next tasks: T002 obtain a normally booted Steam reference, mod/verification history and baseline configuration; T003 close KH2/shared/language source dependencies and independently validate one original/HD payload sample; T004 qualify the supported source profile, then implement P06. Do not substitute a synthetic game loop or a placeholder KH2 viewer for these missing measurements.
