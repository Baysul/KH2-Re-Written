# Roadmap status and work queue

Updated 2026-10-07. The [user-supplied roadmap](https://docs.google.com/document/d/1lewKD-h2ZVOOES1qugUgj82QVMybWwb8sqK3ed-4wPI/edit?tab=t.0) is the acceptance authority. Core scope is confirmed: KH2FM only, installed Steam source. No final gate is represented as passed where user review, reference evidence or dependencies are missing. Source inspection/build/records below are the expressly allowed fallback while P00 is draft.

| Gate | Objective | Status / evidence / dependency |
|---|---|---|
| P00 | Scope and fidelity contract | Draft authored; core scope confirmed; display/controller/tolerances, vendor-save and voice decisions pending |
| P01 | Installed-source and release boundary | Policy authored; candidate scanner source/output isolation tested; launch/cache/release enforcement pending |
| P02 | Installed edition/build identity | 13 files / 16.4 GB hashed twice identically; edition/integrity/shared/language closure unqualified |
| P03 | Windows/Rust/Bevy environment | Build/image/window/timed exit and interactive Escape passed; Steam reference title observed (T002); controller/settings/second environment pending |
| P04 | Durable repository/evidence | Instructions/records/CI/provenance authored; external bundle restored, clean checkout built/tested and synthetic smoke passed (T001R); user review/visibility/second environment pending |
| P05 | Pinned tool/license audit | Six HEDs/47 plain originals/34 BAR tables checked (T003); 23 original storage + 8 HD samples match independent reference (T005); full closure/license audit gaps remain |
| P06 | Native folder picker and source validation | Not started; supported edition/file profile P02 must be qualified; do not enable play on filename evidence |
| P07 | Packages/archives/dependencies | 46,076 headers/candidate names; 82 unique originals/8 HD samples and 19 message/font containers inspected (T003/T005/T007); unsampled/semantic/shared/language closure pending |
| P08 | Read-only importer/source-bound cache | Not started; P06/P07 qualified input required |
| P09 | Reference capture/replay harness | Exploratory private credits/title screenshots and hashed save/config copy (T002); controlled settings/input/time/replay harness pending |
| P10 | Installed executable/runtime research | Mixed-PE audit (T009); 597 declarations / 439 source-mapped CIL bodies (T010); native engine/getters (T011), bounded MCP/project qualification (T012), PE/CLR entry/profile (T013), startup branches (T014), static-slot application entry candidate (T015); cumulative seven listings / 325 independent instructions; full initialization/helper graph, live indirect destinations, active callers and controlled runtime observations pending |
| P11 | Authoritative simulation boundary | Not started; measured time/input/state contract required |
| P12 | Traceable decoded representation | Bounded storage/BAR/message/font readers; 16,765 encoded records and 30 font planes independently compared (T007/T008); glyph/model/script/animation semantics and round-trip corpus pending |
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
| P25 | Menus/HUD/settings/EN/JA/fonts | 30 font palette planes/5,750,784 pixels and 18 spacing byte tables checked; Latin and kana/kanji atlas previews observed (T008); glyph mapping, in-game rendering, full languages and paired behavior pending |
| P26 | Audio/music/synchronization | Not started; decoded sources and measured scheduling required |
| P27 | Every world/encounter/mode | Not started; complete discovered denominator and implementation required |
| P28 | Campaign and both text options | Not started; full P27 route ledger and user play/review required |
| P29 | Steam changes/recovery | Not started; complete profile/cache/save implementations required |
| P30 | Performance/Windows reliability | Not started; full candidate and measured hardware matrix required |
| P31 | Windows package/release qualification | Not started; all required gates and separate user release decision required |

Next tasks: T002 establish a current controlled reference session/settings and mod/verification history; map encoded message symbols/commands to the actual decoded glyph/spacing data, then rooms and KH2/shared/language closure. T015 qualifies a static-slot application entry candidate; follow recorded initialization calls and locate actual global writes/locale inputs, keeping file values distinct from live rebinding. Use current source validation/oracle and explicit close for bridge projects. T004 qualify the supported source profile, then implement P06. Do not substitute a synthetic game loop or a placeholder KH2 viewer for missing measurements.
