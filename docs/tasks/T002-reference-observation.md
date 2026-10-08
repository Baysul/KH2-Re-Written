# T002 — normally launched Steam reference

Owner Codex; reference observed 2026-10-04 UTC / 2026-10-03 local. Gates P03/P09 prerequisite only. User launched the installed KH2FM through Steam after direct startup failed. Candidate source identity remains the privately bound T001/T003 manifest, unqualified; rewrite inspection commit `8ac17e8`.

PASS: a KH2FM executable-owned window is running and visibly presents an English credits video. The window was initially minimized and was restored using Computer Use. This resolves the earlier absence/crash as a current reference-access blocker; it does not explain the direct-launch failure or qualify the edition.

PASS: four vendor save/config files, 7,122,563 bytes, copied before input to `%LOCALAPPDATA%/KH2FinalRewrite/research/reference-20261003/vendor-save-before-input/`; each copied SHA-256 equals its source. Private `save-backup-receipt.json` retains paths/identities. This is a live-process copy; an atomic multi-file snapshot is not asserted. No original file was changed by the copy.

Exploratory keyboard inputs Return (00:08:27.060Z), space (00:08:38.475Z), Escape (00:08:55.727Z) on 2026-10-04 were each followed immediately by a fresh screenshot. All observations still showed credits video. Input delivery does not establish that the game consumed the keys; no input timing, skip binding or transition latency is inferred. Window capture includes chrome; no client-pixel or refresh-rate claim is made.

PASS: later fresh observation shows the KH2 Final Mix title menu with New Game selected and Load disabled. `reference-observation.jpg` / `.json` retain an earlier credits observation privately; `title-reference.jpg` retains the title screenshot. These are exploratory screenshots, not a frame-accurate recording. The title visibly advertises Escape for Game Settings, but injected Escape and one click did not produce an observed settings transition. The user has been asked to open that menu manually and report mod history. No game state was loaded or created by the agent.

NOT RUN: settings/controller mapping, gameplay, save round-trip, frame/audio timing, reference replay or rewrite equivalence. Clean/mod status remains unknown. Next: observe manually opened settings, establish actual display/control baseline, then capture a controlled scenario before gameplay comparison. Public records contain no game pixels or save data.
