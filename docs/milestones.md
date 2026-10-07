# Roadmap status and work queue

Updated 2026-10-07. The [user-supplied roadmap](https://docs.google.com/document/d/1lewKD-h2ZVOOES1qugUgj82QVMybWwb8sqK3ed-4wPI/edit?tab=t.0) is the acceptance authority. Core scope is confirmed: KH2FM only, installed Steam source. No final gate is represented as passed where user review, reference evidence or dependencies are missing. Source inspection/build/records below are the expressly allowed fallback while P00 is draft.

| Gate | Objective | Status / evidence / dependency |
|---|---|---|
| P00 | Scope and fidelity contract | Draft authored; core scope confirmed; user reports never modded; display/controller/tolerances, vendor-save and voice decisions pending |
| P01 | Installed-source and release boundary | Policy authored; candidate scanner source/output isolation tested; launch/cache/release enforcement pending |
| P02 | Installed edition/build identity | Repeated complete scans of 13 files / 16.4 GB agree, including T028 fresh probe admission/preflight/final hashes/sizes/timestamps; user reports never modded; edition/integrity/shared/language closure unqualified |
| P03 | Windows/Rust/Bevy environment | Build/image/window/timed exit and interactive Escape passed; Steam reference title observed (T002); controller/settings/second environment pending |
| P04 | Durable repository/evidence | Instructions/records/CI/provenance authored; external bundle restored, clean checkout built/tested and synthetic smoke passed (T001R); user review/visibility/second environment pending |
| P05 | Pinned tool/license audit | Six HEDs/47 plain originals/34 BAR tables checked (T003); 23 original storage + 8 HD samples match independent reference (T005); full closure/license audit gaps remain |
| P06 | Native folder picker and source validation | Not started; supported edition/file profile P02 must be qualified; do not enable play on filename evidence |
| P07 | Packages/archives/dependencies | 46,076 headers/candidate names; 187 unique originals/26 HD samples after T022's complete replacement reading in 14 font dependencies; unsampled/semantic/shared/language closure pending |
| P08 | Read-only importer/source-bound cache | Not started; P06/P07 qualified input required |
| P09 | Reference capture/replay harness | Exploratory private credits/title screenshots and hashed save/config copy (T002); controlled settings/input/time/replay harness pending |
| P10 | Installed executable/runtime research | Mixed-PE audit (T009); 597 declarations / 439 source-mapped CIL bodies (T010); native engine/getters (T011), bounded MCP/project qualification (T012), PE/CLR entry/profile (T013), startup branches (T014), static-slot application entry candidate (T015), initializer/getter structural linkage (T016); cumulative ten listings / 609 independent instructions; nested initialization/language meanings, live destinations, active callers and controlled runtime observations pending |
| P11 | Authoritative simulation boundary | Not started; measured time/input/state contract required |
| P12 | Traceable decoded representation | Bounded readers; 81,272 records / 4,231,983 tokens (T017), explicit International profile / 4,092,396 scalars with 51 unknown slots (T018), font planes/cells (T008/T020), public byte-exact re-encoding (T021); 15 PNGs / 13,107,200 pixels match native decoding, default DDS comparisons FAIL (T023); explicit normalized BC3 profile matches both boundary oracles and fresh WIC for all 19 selected images (T025), legacy FAIL preserved; T027 independently matches channel distributions/root IMGD header/link; T028 matches its indexed-8 linear CPU pixels/palette; installed routing, command conflict, other formats and full corpus pending |
| P13 | Asset/animation inspection app | Started: T026 source-bound texture/channel viewer displays 19 actual images in 114 states; 81,469,440 visible pixels independently match. T028 privately inspects an independently matched original IMGD preview. Tall native atlases cropped; physical controls/fit/zoom, geometry and animation pending |
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
| P25 | Menus/HUD/settings/EN/JA/fonts | 30 font planes and 18 spacing tables checked (T008); 18 pairs/6,772 cells matched (T020); all 19 font replacements storage-verified (T022), 15 PNGs pixel-exact (T023); explicit normalized BC3 profile matches fresh WIC on all four DDSs (T025), default retains historical FAIL; installed packed channels/rounding, HD UV/advances, full languages and paired behavior pending |
| P26 | Audio/music/synchronization | Not started; decoded sources and measured scheduling required |
| P27 | Every world/encounter/mode | Not started; complete discovered denominator and implementation required |
| P28 | Campaign and both text options | Not started; full P27 route ledger and user play/review required |
| P29 | Steam changes/recovery | Not started; complete profile/cache/save implementations required |
| P30 | Performance/Windows reliability | Not started; full candidate and measured hardware matrix required |
| P31 | Windows package/release qualification | Not started; all required gates and separate user release decision required |

Next tasks: prioritize T002 controlled reference settings and Steam verification/independent integrity, plus T004 supported-source qualification before P06. User reports never modded. P00 choices remain pending. T019's reference snapshot found Steam running and KH2FM absent. T020 independently matches font-cell candidates; T021 qualifies public re-encoding, retaining all 51 unknown slots. T023 verifies 15 PNGs pixel-exact and records all four default DDS/WIC disagreements. T025 integrates the explicit normalized BC3 CPU profile; actual library boundary pixels and all 19 selected source images match independently. T026 supplies actual source-bound texture/channel inspection with exact visible-region captures, without installed font/shader qualification. T027 independently matches full channel distributions and resolves the root lead as IMGD, correcting the TIM2 suggestion. T028 matches the explicit original indexed-8 linear CPU profile and paired dimensions without assuming HD mapping. Next qualify additional original image/geometry formats from actual assets and extend inspection toward qualified geometry/animation. Preserve runtime font/advance uncertainty; continue the command-length conflict and Japanese/shared closure. T016's nested initialization, actual locale inputs and runtime behavior remain unknown. Use current source validation/oracle and explicit close for bridge projects. Do not substitute a synthetic game loop or a placeholder KH2 viewer for missing measurements.
