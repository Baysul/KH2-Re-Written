# T082 — Remove the field's X mirror

Owner Claude, 2026-10-09. Gate P05/P22 presentation. Starts from T081.

User report (2026-10-09): "everything for models is reversed".

## Source

The test room drew every scene entity under a root scaled by `MIRROR` = (−1, 1, 1). That was an inferred choice (T056-era) to put the Kingdom Key in Sora's right hand. It was not an observed render matrix.

The exe's own model path has no mirror (`camp-03/menuobj-spec.md` §1.2–1.4, private):
- the kn::Camera view is built from eye/target/up in the Y-down game world;
- the projection maps camera x to screen right;
- the 3D vertex shader only flips y for D3D;
- model +X therefore ends on screen right.

The room's `from_world` (x, −y, −z) is a rotation about X, so raw model coordinates with that rotation already match the exe.

## Implementation

- The scene root is now unscaled.
- The follow camera, the talk camera and the camera-relative stick no longer multiply by `MIRROR`; the constant is gone.
- Effects already convert through the same view, unchanged.

## Evidence (private)

- `autoplay-70`: arena-06 through `kh2fm room`, the full walk-through including combat, guard, camp, Items and Abilities. Its captures show the room and Sora unmirrored.
- `talk-autoplay-04`: Axel unmirrored.

## Checks

PASS fmt; PASS clippy; PASS tests; PASS build; PASS smoke; PASS autoplay-70 and talk-autoplay-04 (exit 0, no uniform captures).

NOT RUN: a side-by-side capture against the installed game.

## Uncertainty

- Which hand holds the Kingdom Key now follows the data. The objentry weapon joint (25) is used as read.
- The PC field camera's own view/projection was not traced separately; this follows the camp model path and the shared VS.

## Verdict / next

PASS for the report. Next: Party / Customize / Equip screens.
