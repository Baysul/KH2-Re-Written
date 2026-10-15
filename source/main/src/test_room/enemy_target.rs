//! Stable live actor/shape targeting. The status panel is an explicit debug
//! presentation; native lock/Scan sprites and layout are not substituted.
use super::*;
use player::targeting::{self, Candidate, ManualCandidate, ManualInput, StableHolder, State};

pub const SCAN_ITEM: u16 = 138;
pub const SCAN_BIT: u16 = 19;

pub fn set_scan(game: &mut Game, camp: Option<&mut CampRes>, enabled: bool) -> Result<(), String> {
    if !game
        .ability_source
        .items
        .get(&SCAN_ITEM)
        .is_some_and(|&(bit, _)| bit == SCAN_BIT)
    {
        return Err("Selected-source Scan item/ability binding differs".into());
    }
    set_scan_slots(&mut game.ability_slots, enabled)?;
    if let Some(camp) = camp {
        camp.ability_slots.clone_from(&game.ability_slots);
    }
    drive_room::rebuild_abilities(game, None);
    Ok(())
}

fn set_scan_slots(slots: &mut [u16], enabled: bool) -> Result<(), String> {
    let mut found = false;
    for slot in slots.iter_mut().filter(|slot| **slot & 0x7fff == SCAN_ITEM) {
        *slot = SCAN_ITEM | if enabled { 0x8000 } else { 0 };
        found = true;
    }
    if found {
        Ok(())
    } else {
        Err("Scan is not present in the current learned ability inventory".into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetId {
    Legacy(usize),
    Gallery(u64, usize),
}

#[derive(Resource, Default)]
pub struct Targeting {
    pub holder: StableHolder<TargetId>,
    pub limitation: Option<&'static str>,
    queued_cycle: Option<f32>,
}

impl Targeting {
    /// Preserve the native cycle direction if input is released before the
    /// next fixed simulation update consumes its queued request.
    pub fn queue_cycle(&mut self, logical_pressed: u64) {
        if let Some(direction) = cycle_direction(logical_pressed) {
            self.queued_cycle = Some(direction);
        }
    }
}

fn cycle_direction(logical: u64) -> Option<f32> {
    if logical & (1 << 36) != 0 {
        Some(-1.)
    } else if logical & (1 << 37) != 0 {
        Some(1.)
    } else {
        None
    }
}

pub struct Selection {
    pub shapes: Vec<player::ptya::Target>,
    pub target: Option<player::ptya::Target>,
    pub id: Option<TargetId>,
    pub locked: bool,
}

pub fn player_targetcentre(game: &Game) -> Option<[f32; 3]> {
    let Some(shape) = game.player_target_shape.as_ref() else {
        return Some(game.player.position);
    };
    let bone = usize::try_from(shape.bone).ok()?;
    let matrix = if bone == 0x4000 {
        Mat4::IDENTITY
    } else {
        let index = *game.poses.get(&(game.player.action, game.player.armed))?;
        let clip = game.clip_poses.get(index)?;
        let frame = game.player.motion_frame(&game.clips) as usize;
        *clip
            .body
            .get(frame.min(clip.body.len().saturating_sub(1)))?
            .get(&bone)?
    };
    let offset = Vec3::from_array(shape.position.map(|value| value as f32));
    let local = matrix.transform_point3(offset) + Vec3::Y * shape.position_height as f32;
    let centre =
        Vec3::from_array(game.player.position) + Quat::from_rotation_y(game.player.yaw) * local;
    centre.is_finite().then(|| centre.to_array())
}

pub fn candidates(game: &Game, gallery: &enemy_gallery::EnemyGallery) -> Vec<Candidate<TargetId>> {
    let mut out = Vec::new();
    if let Some(rt) = &game.enemy
        && rt.enabled
        && rt.enemy.targetable()
        && let Some(clip) = rt.clips.0.get(&rt.enemy.motion)
        && let Some(bones) = clip
            .bones
            .get((rt.enemy.motion_tick as usize).min(clip.bones.len().saturating_sub(1)))
    {
        let origin = Vec3::from_array(rt.enemy.position);
        let turn = Quat::from_rotation_y(rt.enemy.yaw);
        for (ordinal, &(bone, offset, radius, height)) in rt.targets.iter().enumerate() {
            if let Some(matrix) = bones.get(&bone) {
                out.push(Candidate {
                    key: TargetId::Legacy(ordinal),
                    shape: player::ptya::Target {
                        centre: (origin + turn * matrix.transform_point3(offset)).to_array(),
                        radius,
                        height,
                    },
                });
            }
        }
    }
    for actor in &gallery.actors {
        if let Some(battle) = &actor.battle
            && battle.error.is_none()
            && battle.brain.targetable()
        {
            out.extend(battle.pose.targets.iter().map(|shape| Candidate {
                key: TargetId::Gallery(actor.id, shape.ordinal),
                shape: player::ptya::Target {
                    centre: shape.centre.to_array(),
                    radius: shape.radius,
                    height: shape.height,
                },
            }));
        }
    }
    out
}

pub fn select(
    game: &Game,
    gallery: &enemy_gallery::EnemyGallery,
    state: &mut Targeting,
    toggle: bool,
    cycle: bool,
    dt: f32,
) -> Selection {
    let candidates = candidates(game, gallery);
    let cycle_direction = if cycle {
        state
            .queued_cycle
            .take()
            .or_else(|| cycle_direction(game.pad.pressed | game.pad.held))
    } else {
        None
    };
    state.limitation = None;
    if let Some(params) = &gallery.target_params {
        let mut manual = ManualInput {
            toggle,
            initial_choice: None,
            cycle_choice: None,
        };
        let initial = toggle && state.holder.state == State::None;
        if initial || (cycle && state.holder.state == State::Locked) {
            if let (Some(origin), Some(range)) = (player_targetcentre(game), gallery.lock_range) {
                let shapes = candidates
                    .iter()
                    .map(|&candidate| ManualCandidate {
                        candidate: Candidate {
                            key: candidate.key,
                            shape: player::ptya::Target {
                                centre: to_world(candidate.shape.centre),
                                ..candidate.shape
                            },
                        },
                        // Ordinary development-room actors are constructed without
                        // the native special range-bypass flag. Boss hosts differ.
                        bypass_range: false,
                    })
                    .collect::<Vec<_>>();
                if initial {
                    manual.initial_choice =
                        targeting::choose_manual(to_world(origin), range, &shapes);
                } else if let Some(current) = state.holder.target {
                    if let Some(direction) = cycle_direction {
                        manual.cycle_choice = targeting::cycle_manual(
                            to_world(origin),
                            range,
                            current,
                            direction,
                            &shapes,
                        );
                    } else {
                        state.limitation = Some("Manual cycle input producer unqualified");
                    }
                }
            } else {
                state.limitation = Some("Source manual target origin/range unavailable");
            }
        }
        state.holder.update(
            params,
            game.player.position,
            [game.player.yaw.sin(), game.player.yaw.cos()],
            &candidates,
            manual,
            dt,
        );
    } else {
        state.holder = StableHolder::default();
        state.limitation = Some("Selected-source player targeting parameters unavailable");
    }
    selection(state, &candidates)
}

fn selection(state: &Targeting, candidates: &[Candidate<TargetId>]) -> Selection {
    let index = state.holder.index(candidates);
    let target = index.map(|index| candidates[index].shape);
    let mut shapes = candidates
        .iter()
        .map(|candidate| candidate.shape)
        .collect::<Vec<_>>();
    if let Some(index) = index {
        shapes.swap(0, index);
    }
    Selection {
        shapes,
        target,
        id: state.holder.target,
        locked: state.holder.state == State::Locked,
    }
}

/// Source attack acquisition/hold, preserving a manual lock's stable identity.
pub fn acquire(
    game: &Game,
    gallery: &enemy_gallery::EnemyGallery,
    state: &mut Targeting,
) -> Selection {
    let candidates = candidates(game, gallery);
    if let Some(params) = &gallery.target_params {
        state.holder.acquire(
            params,
            game.player.position,
            [game.player.yaw.sin(), game.player.yaw.cos()],
            &candidates,
        );
        state.holder.hold(params);
    }
    selection(state, &candidates)
}

pub fn camera_anchor(
    game: &Game,
    gallery: &enemy_gallery::EnemyGallery,
    id: TargetId,
) -> Option<[f32; 3]> {
    match id {
        TargetId::Gallery(actor, ordinal) => {
            let battle = gallery
                .actors
                .iter()
                .find(|a| a.id == actor)?
                .battle
                .as_ref()?;
            battle
                .pose
                .camera
                .first()
                .or_else(|| battle.pose.targets.iter().find(|s| s.ordinal == ordinal))
                .map(|s| s.centre.to_array())
        }
        TargetId::Legacy(ordinal) => game
            .enemy
            .as_ref()
            .and_then(|rt| enemy_targets(rt).get(ordinal).map(|s| s.centre)),
    }
}

fn name_and_hp(
    game: &Game,
    gallery: &enemy_gallery::EnemyGallery,
    id: TargetId,
) -> Option<(Vec<u8>, i32, i32)> {
    match id {
        TargetId::Legacy(_) => {
            let rt = game
                .enemy
                .as_ref()
                .filter(|rt| rt.enabled && rt.enemy.targetable())?;
            let name = gallery
                .catalog
                .entries
                .iter()
                .find(|entry| entry.object_id == 302)
                .and_then(|entry| entry.name_bytes.clone())
                .unwrap_or_else(|| pause_menu::encode_text("Shadow"));
            Some((name, rt.enemy.hp, rt.enemy.stats.max_hp))
        }
        TargetId::Gallery(id, _) => {
            let actor = gallery.actors.iter().find(|actor| actor.id == id)?;
            let battle = actor
                .battle
                .as_ref()
                .filter(|battle| battle.brain.targetable())?;
            let entry = &gallery.catalog.entries[actor.entry];
            let name = entry
                .name_bytes
                .clone()
                .unwrap_or_else(|| pause_menu::encode_text(&entry.name));
            Some((name, battle.brain.hp(), battle.assets.stats.max_hp))
        }
    }
}

fn rect(bounds: [f32; 4], rgba: [f32; 4]) -> sequence::Quad {
    let [x0, y0, x1, y1] = bounds;
    sequence::Quad {
        corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
        uv: [0, 0, 1, 1],
        colors: [rgba; 4],
        blend: 0,
    }
}

/// The caller supplies the qualified Scan ability bit; None never guesses an
/// ID or exposes HP. Uses the existing source font and a plain debug panel.
pub fn draws(
    game: &Game,
    gallery: &enemy_gallery::EnemyGallery,
    state: &Targeting,
    white: usize,
    font_sheet: usize,
    widths: &[u8],
    scan_ability: Option<u16>,
) -> Vec<Draw> {
    let mut label = pause_menu::encode_text(if state.holder.state == State::Locked {
        "LOCK ON: "
    } else {
        "Target: "
    });
    let Some((name, hp, max_hp)) = state
        .holder
        .target
        .and_then(|id| name_and_hp(game, gallery, id))
    else {
        return if state.limitation.is_some() {
            status_draws(
                &StatusView {
                    label: &pause_menu::encode_text("Target unavailable"),
                    hp: 0,
                    max_hp: 0,
                    scan_enabled: false,
                    limitation: state.limitation,
                },
                white,
                font_sheet,
                widths,
            )
        } else {
            Vec::new()
        };
    };
    label.extend(name);
    let equipped = scan_ability == Some(SCAN_BIT)
        && game
            .ability_source
            .items
            .get(&SCAN_ITEM)
            .is_some_and(|&(bit, _)| bit == SCAN_BIT)
        && game.player.abilities.has(SCAN_BIT);
    let primary_pool = match state.holder.target {
        Some(TargetId::Gallery(id, ordinal)) => gallery
            .actors
            .iter()
            .find(|actor| actor.id == id)
            .and_then(|actor| actor.battle.as_ref())
            .and_then(|battle| {
                battle
                    .pose
                    .targets
                    .iter()
                    .find(|shape| shape.ordinal == ordinal)
            })
            .is_some_and(|shape| shape.parts == 0),
        Some(TargetId::Legacy(_)) => true,
        None => false,
    };
    let limitation = state.limitation.or(if equipped && !primary_pool {
        Some("Secondary target HP pool unqualified")
    } else {
        None
    });
    status_draws(
        &StatusView {
            label: &label,
            hp,
            max_hp,
            scan_enabled: equipped && primary_pool,
            limitation,
        },
        white,
        font_sheet,
        widths,
    )
}

struct StatusView<'a> {
    label: &'a [u8],
    hp: i32,
    max_hp: i32,
    scan_enabled: bool,
    limitation: Option<&'static str>,
}

fn status_draws(
    view: &StatusView<'_>,
    white: usize,
    font_sheet: usize,
    widths: &[u8],
) -> Vec<Draw> {
    let font = command::Font { widths, pad: 1 };
    let mut quads = vec![rect([16., 42., 256., 83.], [0.03, 0.04, 0.07, 0.78])];
    let size = 13_f32.min(13. * 226. / command::text_width(view.label, &font, 13.).max(1.));
    let mut glyphs = command::text_quads_sized(view.label, [22., 46.], [1.; 4], &font, size);
    if view.scan_enabled {
        let fraction = if view.max_hp > 0 {
            view.hp.max(0) as f32 / view.max_hp as f32
        } else {
            0.
        }
        .clamp(0., 1.);
        quads.push(rect([22., 69., 170., 76.], [0.08, 0.12, 0.12, 1.]));
        quads.push(rect(
            [22., 69., 22. + 148. * fraction, 76.],
            [0.2, 0.95, 0.35, 1.],
        ));
        glyphs.extend(command::text_quads_sized(
            &pause_menu::encode_text(&format!("{}/{}", view.hp, view.max_hp)),
            [176., 64.],
            [1.; 4],
            &font,
            11.,
        ));
    }
    let caption = view
        .limitation
        .unwrap_or("Target status (debug presentation)");
    let caption = pause_menu::encode_text(caption);
    let caption_size = 10_f32.min(10. * 226. / command::text_width(&caption, &font, 10.).max(1.));
    glyphs.extend(command::text_quads_sized(
        &caption,
        [22., 84.],
        [0.8, 0.85, 0.9, 1.],
        &font,
        caption_size,
    ));
    vec![
        Draw::Quads {
            quads,
            z: 0,
            sheet: white,
        },
        Draw::Quads {
            quads: glyphs,
            z: 0,
            sheet: font_sheet,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_cycle_direction_survives_release_before_fixed_update() {
        let mut state = Targeting::default();
        state.queue_cycle(1 << 36);
        state.queue_cycle(0);
        assert_eq!(state.queued_cycle.take(), Some(-1.));
        assert_eq!(state.queued_cycle.take(), None);
        state.queue_cycle((1 << 36) | (1 << 37));
        assert_eq!(state.queued_cycle.take(), Some(-1.));
        state.queue_cycle(1 << 37);
        assert_eq!(state.queued_cycle.take(), Some(1.));
    }

    #[test]
    fn scan_toggle_preserves_other_slots_and_never_grants_unlearned_abilities() {
        let mut slots = [0x8001, SCAN_ITEM, 0x8123, SCAN_ITEM | 0x8000];
        set_scan_slots(&mut slots, false).unwrap();
        assert_eq!(slots, [0x8001, SCAN_ITEM, 0x8123, SCAN_ITEM]);
        set_scan_slots(&mut slots, true).unwrap();
        assert_eq!(
            slots,
            [0x8001, SCAN_ITEM | 0x8000, 0x8123, SCAN_ITEM | 0x8000]
        );
        let mut unlearned = [0x8001, 0, 0x8123];
        assert!(set_scan_slots(&mut unlearned, true).is_err());
        assert_eq!(unlearned, [0x8001, 0, 0x8123]);
    }

    #[test]
    fn unequipped_scan_hides_hp_and_equipped_scan_uses_live_hp_ratio() {
        let name = pause_menu::encode_text("LOCK ON: Synthetic");
        let mut view = StatusView {
            label: &name,
            hp: 25,
            max_hp: 100,
            scan_enabled: false,
            limitation: None,
        };
        let off = status_draws(&view, 0, 1, &[10; 224]);
        let Draw::Quads { quads, .. } = &off[0] else {
            panic!("status background missing")
        };
        assert_eq!(quads.len(), 1);
        view.scan_enabled = true;
        let on = status_draws(&view, 0, 1, &[10; 224]);
        let Draw::Quads { quads, .. } = &on[0] else {
            panic!("status background missing")
        };
        assert_eq!(quads.len(), 3);
        assert_eq!(quads[2].corners[1][0], 59.);
        view.hp = 50;
        let changed = status_draws(&view, 0, 1, &[10; 224]);
        let Draw::Quads { quads, .. } = &changed[0] else {
            panic!("status background missing")
        };
        assert_eq!(quads[2].corners[1][0], 96.);
    }
}
