//! The command manager's reaction command in the room (FUN_1403b2340): each
//! tick, before Sora's update, one search over everything that may fill the
//! Triangle slot (`player::reaction_command`, FUN_1404081c0), the slot it
//! keeps with its target marker (FUN_140408520), then Triangle. What wins
//! decides who carries a press out:
//!
//! - a battle actor's REACTION shape or attack: its script gets trigger 11
//!   (FUN_1403a8b10 → FUN_1403b5fa0) in the same tick;
//! - the Talk NPC's shape: the field talk starts (`talk_tick`);
//! - the manager's record 1 (a Limit Form limit's, a summon's or a party
//!   limit's follow-up): that module's follow-up;
//! - Auto Summon, an auto form or Auto Limit: the summon, the form or the
//!   limit starts as the deck's command would.
//!
//! A taken command is held (+0xdd8, FUN_1403a89a0) and shows in mode 8 until
//! another source outranks it or Sora's next command or hit clears it.
use super::*;
use enemy_gallery::{EnemyGallery, GalleryActor};
use player::reaction_command as rc;

/// Keys of the room's non-actor offerers in the search (actor ids are
/// small).
pub const SORA: u32 = 0xffff_0000;
pub const TALK: u32 = 0xffff_0001;
pub const SUMMON: u32 = 0xffff_0002;
/// The townspeople's and the test NPC's reaction (cmd 31 "Talk").
const TALK_COMMAND: u16 = 0x1f;

/// An auto ability's `item` record (03system, 0x18-byte rows): its kind
/// (+2), its status bit (+4) and its command (+0x10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoRecord {
    pub kind: u8,
    pub bit: u16,
    pub command: u16,
}

/// The command records and names the search reads (03system `cmd`), and the
/// auto abilities' item records.
pub struct Commands {
    pub rows: HashMap<u16, rc::Row>,
    pub names: HashMap<u16, Vec<u8>>,
    pub autos: HashMap<u16, AutoRecord>,
}

/// The records of 03system `cmd`, the names (messages) of the commands that
/// can show in the slot, and the auto items' records.
pub fn load(
    system: &[u8],
    names: impl FnOnce(&[u32]) -> Result<Vec<Option<Vec<u8>>>, String>,
) -> Result<Commands, String> {
    let table = data::command_table::read(bar_entry(system, b"cmd\0").map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let rows: HashMap<u16, rc::Row> = table
        .iter()
        .map(|c| {
            (
                c.id,
                rc::Row {
                    id: c.id,
                    exec: c.execute,
                    argument: c.argument,
                    submenu: c.submenu,
                    flags: c.flags,
                    facing_cos: c.reaction.facing_cos,
                    target_yaw: c.reaction.target_yaw,
                    target_cos: c.reaction.target_cos,
                    cost: c.cost,
                    camera: c.reaction.camera,
                    priority: c.reaction.priority,
                    decider: c.decider,
                    chain: c.chain,
                    mark: c.reaction.mark,
                    action: c.reaction.action,
                    distance: c.reaction.distance,
                    form_mask: c.form_mask,
                    group: c.group,
                },
            )
        })
        .collect();
    let named: Vec<&data::command_table::Command> = table.iter().filter(|c| c.name != 0).collect();
    let ids: Vec<u32> = named.iter().map(|c| c.name).collect();
    let texts = names(&ids)?;
    let names = named
        .iter()
        .zip(texts)
        .filter_map(|(c, t)| Some((c.id, t?)))
        .collect();
    let mut autos = HashMap::new();
    if let Ok(items) = bar_entry(system, b"item") {
        let count = items
            .get(4..8)
            .map_or(0, |w| u32::from_le_bytes(w.try_into().unwrap()) as usize);
        let wanted: Vec<u16> = rc::AUTO_FORMS
            .iter()
            .chain(&rc::AUTO_SUMMONS)
            .copied()
            .chain([rc::AUTO_LIMIT, AUTO_SUMMON])
            .collect();
        for r in items
            .get(8..)
            .unwrap_or_default()
            .as_chunks::<0x18>()
            .0
            .iter()
            .take(count)
        {
            let id = u16::from_le_bytes([r[0], r[1]]);
            if wanted.contains(&id) {
                autos.insert(
                    id,
                    AutoRecord {
                        kind: r[2],
                        bit: u16::from_le_bytes([r[4], r[5]]),
                        command: u16::from_le_bytes([r[0x10], r[0x11]]),
                    },
                );
            }
        }
    }
    Ok(Commands { rows, names, autos })
}

/// The Auto Summon ability (item 0x185: status bit 56, FUN_140407a90).
pub const AUTO_SUMMON: u16 = 0x185;

/// The manager's reaction state across ticks.
pub struct State {
    pub slot: rc::Slot,
    /// +0xdd8 / +0xbf8: the command taken last and its target.
    pub hold: Option<(u16, rc::Target)>,
    /// The search's winner this tick.
    pub offer: Option<rc::Offer>,
    /// A command Triangle took this tick, for its source to carry out.
    pub press: Option<rc::Candidate>,
    /// `DAT_140783ca0` for the auto offers.
    pub seed: u32,
    /// The target point of each shape offered this tick (for the marker).
    pub points: Vec<(rc::Target, [f32; 3])>,
    /// The Talk NPC's label.
    pub talk_label: Vec<u8>,
    /// The last marker the run log recorded.
    pub marker_logged: u32,
    /// Since the hold began Sora left idle / walk / run, or talked.
    hold_away: bool,
    hold_talk: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            slot: rc::Slot::default(),
            hold: None,
            offer: None,
            press: None,
            seed: 1,
            points: Vec::new(),
            talk_label: Vec::new(),
            marker_logged: 0,
            hold_away: false,
            hold_talk: false,
        }
    }
}

impl State {
    /// The press this tick if it took `command` from `source`.
    pub fn pressed(&self, source: rc::Source) -> Option<u16> {
        self.press.filter(|p| p.source == source).map(|p| p.command)
    }

    /// The press this tick on a shape of `owner`.
    pub fn pressed_on(&self, owner: u32) -> Option<u16> {
        self.press
            .filter(|p| p.source == rc::Source::Shape && p.target.owner == owner)
            .map(|p| p.command)
    }

    /// The slot's command for the HUD.
    pub fn shown(&self) -> Option<rc::Candidate> {
        self.slot.shown.valid().then_some(self.slot.shown)
    }
}

/// The search's view of the room.
struct World<'a> {
    commands: &'a Commands,
    actors: &'a [GalleryActor],
    /// FUN_140401d60's cases with state: the room's colour of the Drive,
    /// summon and limit commands.
    modes: HashMap<u16, u8>,
    abilities: &'a player::abilities::Abilities,
    stock: Option<&'a std::collections::BTreeMap<u16, u8>>,
}

impl rc::Env for World<'_> {
    fn row(&self, id: u16) -> Option<rc::Row> {
        self.commands.rows.get(&id).copied()
    }

    fn exec_mode(&self, row: &rc::Row) -> u8 {
        self.modes
            .get(&row.id)
            .copied()
            .unwrap_or_else(|| rc::exec_mode(row))
    }

    fn offered(&self, shape: &rc::Shape) -> bool {
        if shape.owner == TALK {
            return true;
        }
        self.actors
            .iter()
            .find(|a| a.id as u32 == shape.owner)
            .and_then(|a| a.battle.as_ref())
            .is_some_and(|b| {
                b.brain.reaction_offered(
                    i32::from(shape.command),
                    i32::from(shape.group),
                    b.assets.default_groups,
                )
            })
    }

    fn inactive(&self, owner: u32) -> bool {
        if owner >= SORA {
            return false;
        }
        self.actors
            .iter()
            .find(|a| a.id as u32 == owner)
            .and_then(|a| a.battle.as_ref())
            .is_none_or(|b| b.error.is_some() || !b.brain.reaction_active())
    }

    /// FUN_1403e39a0 with FUN_1403d7470 (an ability's status bit) and
    /// FUN_1403c4720 (an item owned).
    fn auto_item(&self, item: u16) -> Option<rc::AutoItem> {
        let r = self.commands.autos.get(&item)?;
        let held = match r.kind {
            0x13 => self.abilities.has(r.bit),
            _ => self
                .stock
                .is_some_and(|s| s.get(&item).is_some_and(|&n| n > 0)),
        };
        Some(rc::AutoItem {
            kind: r.kind,
            held,
            command: r.command,
        })
    }
}

/// Sora's FUN_1403ba070 state as the room keeps it: an ANB range 0..2 or a
/// reaction's tail for the window, range 26 for the state. [U] While a
/// summon's limit scene captures him, the room plays his captured clips
/// without their motion flags (the scene's proxy of him holds none either),
/// so the window counts as open and the no-control test as passed there.
fn sora_actor(game: &Game) -> rc::Actor {
    if summon::in_limit_scene(game) {
        return rc::Actor {
            window: true,
            ..Default::default()
        };
    }
    rc::Actor {
        no_control: game.player.no_control(&game.clips),
        state: game.player.reaction_state(&game.clips),
        window: game.player.can_command(&game.clips),
        ..Default::default()
    }
}

/// FUN_1403b2340's reaction part, every tick before Sora's update.
pub fn tick(
    mut game: ResMut<Game>,
    gallery: Option<ResMut<EnemyGallery>>,
    talk: Option<Res<Talk>>,
    hud: Option<ResMut<Hud>>,
) {
    let tick = game.tick;
    let mut gallery = gallery;
    let actors: &mut [GalleryActor] = match gallery.as_mut() {
        Some(g) => &mut g.actors,
        None => &mut [],
    };
    let Some(commands) = game.reaction_commands.take() else {
        return;
    };
    let mut state = std::mem::take(&mut game.reaction);
    state.press = None;
    if let Some(t) = talk.as_deref() {
        state.talk_label.clone_from(&t.label);
    }
    // The hold ends with Sora's next command or hit: FUN_1403a89a0 clears
    // +0xdd8 for any request, and the state handlers that bring him back to
    // idle call FUN_1403a81c0 [I: the room clears it when he is hurt, comes
    // back to idle, walk or run after leaving them, or ends a talk].
    if state.hold.is_some() {
        use player::Action;
        let free = matches!(
            game.player.action,
            Action::Idle | Action::Walk | Action::Run | Action::Land
        );
        if matches!(game.player.action, Action::Hurt(_))
            || state.hold_away && free
            || state.hold_talk && !game.talking
        {
            state.hold = None;
        }
        state.hold_away |= !free;
        state.hold_talk |= game.talking;
    }
    if state.hold.is_none() {
        state.hold_away = false;
        state.hold_talk = false;
    }
    let menu_kind = hud
        .as_ref()
        .and_then(|h| h.menu.as_ref())
        .map(|m| match m.menu.sub.as_ref() {
            Some(sub) => sub.kind as u8,
            None => m.menu.kind as u8,
        });
    let inputs = inputs(&game, actors, talk.as_deref(), &state, menu_kind);
    let world = World {
        commands: &commands,
        actors,
        modes: exec_modes(&game, &commands),
        abilities: &game.player.abilities,
        stock: game.items.as_ref().map(|i| &i.stock),
    };
    let offer = rc::search(&world, &inputs, &mut state.seed);
    // FUN_140408bd0: a source other than the hold clears it.
    if offer.is_some_and(|o| o.candidate.source != rc::Source::Hold) {
        state.hold = None;
    }
    // FUN_140408520: +0xde0 == 6 (Antiform) shows the slot dimmed.
    let force_dim = game
        .drive
        .as_ref()
        .is_some_and(|d| d.active && d.form == player::forms::ANTI);
    state.slot.update(
        &world,
        offer,
        rc::SlotState {
            force_dim,
            ..Default::default()
        },
    );
    state.points = inputs
        .shapes
        .iter()
        .chain(&inputs.attacks)
        .map(|s| (s.target(), s.centre))
        .collect();
    if offer.map(|o| o.command()) != state.offer.map(|o| o.command()) {
        if offer.is_some() {
            game.offers_seen += 1;
        }
        game.event(serde_json::json!({"event":"reaction_offer","tick":tick,
            "command":offer.map(|o| o.command()),"owner":offer.map(|o| o.candidate.target.owner),
            "source":offer.map(|o| format!("{:?}", o.candidate.source)),
            "priority":offer.map(|o| o.priority),"mode":offer.map(|o| o.candidate.mode)}));
    }
    state.offer = offer;
    // Triangle (logical bit 10), and the tours' presses on their own offers.
    let shown = state.shown();
    let triangle = std::mem::take(&mut game.rc_queued) || tour_press(&game, shown, tick);
    let row = shown.and_then(|c| commands.rows.get(&c.command).copied());
    let sora = sora_actor(&game);
    let battle = inputs.sora.battle;
    let press = rc::press(
        &mut state.slot,
        rc::Manager::default(),
        triangle,
        row.as_ref(),
        |row| {
            // FUN_1403b6950: Sora's windows, the target's (the room's
            // others take commands at any time [I]), or both.
            let target = rc::Actor {
                window: true,
                ..Default::default()
            };
            rc::usable(row, &sora, Some(&target), battle)
        },
    );
    match press {
        Some(rc::Press::Taken { command, .. }) => {
            let c = state.slot.shown;
            state.press = Some(c);
            // FUN_14015d660: the HUD's burst.
            if let Some(mut hud) = hud
                && let Some(m) = hud.menu.as_mut()
            {
                m.menu.burst();
            }
            // FUN_1403a89a0: the taken command is held unless its record's
            // flag bit 9 says not to.
            if let Some(row) = row {
                state.hold = rc::hold_after(&row, true).map(|cmd| (cmd, c.target));
            }
            game.event(serde_json::json!({"event":"reaction_taken","tick":tick,
                "command":command,"source":format!("{:?}", c.source),"owner":c.target.owner}));
            carry_out(&mut game, actors, c, tick);
        }
        Some(rc::Press::Refused) => game.sound_queue.push((5, "menu_denied")),
        Some(rc::Press::Ignored) => {
            // [Run log] Why a shown command was not taken.
            let action = format!("{:?}", game.player.action);
            game.event(serde_json::json!({"event":"reaction_ignored","tick":tick,
                "command":shown.map(|c| c.command),"mode":shown.map(|c| c.mode),
                "flags":row.map(|r| r.flags),"decider":row.map(|r| r.decider),
                "chain":row.map(|r| r.chain),"sora":format!("{sora:?}"),
                "action":action}));
        }
        _ => {}
    }
    game.reaction = state;
    game.reaction_commands = Some(commands);
}

/// The search's inputs from the room.
fn inputs(
    game: &Game,
    actors: &[GalleryActor],
    talk: Option<&Talk>,
    state: &State,
    menu_kind: Option<u8>,
) -> rc::Inputs {
    let mut shapes = Vec::new();
    for actor in actors {
        let Some(battle) = actor.battle.as_ref() else {
            continue;
        };
        if battle.error.is_some() || !battle.brain.reaction_active() {
            continue;
        }
        for &(ref shape, kind, argument) in &battle.pose.reactions {
            // Kind 14 offers the command a clashed attack left (+0xcf0) and
            // argument 0x83 the spawn record's: [U] neither is kept by the
            // room's actors (no clash command, no spawn record).
            if kind != data::object_collision::REACTION {
                continue;
            }
            let Ok(argument) = u16::try_from(argument) else {
                continue;
            };
            let command = rc::shape_command(5, argument, 0, None);
            if command == 0x83 {
                continue;
            }
            shapes.push(rc::Shape {
                owner: actor.id as u32,
                group: shape.group,
                command,
                centre: shape.centre.to_array(),
                radius: shape.radius,
                height: shape.height,
                owner_yaw: actor.yaw,
                attack: None,
            });
        }
    }
    // FUN_140407b40: attacks carrying a command (`attack_set_rc`), a sphere
    // at the attack's position plus its offset, offered for the object that
    // owns it. [U] An owner set by `attack_set_rc_owner` to another object
    // is not resolved to that actor; the facing gate turns with the actor.
    let mut attacks = Vec::new();
    for actor in actors {
        let Some(battle) = actor.battle.as_ref() else {
            continue;
        };
        if battle.error.is_some() {
            continue;
        }
        for a in battle.brain.script_attacks() {
            let Some(r) = a.rc.filter(|r| r.own) else {
                continue;
            };
            let at = Vec3::from_array(a.position) + Vec3::from_array(r.offset);
            attacks.push(rc::Shape {
                owner: actor.id as u32,
                group: 0,
                command: r.command,
                centre: at.to_array(),
                radius: r.radius,
                height: r.height,
                owner_yaw: actor.yaw,
                attack: Some(a.serial as u32),
            });
        }
    }
    // The Talk NPC's REACTION sphere (r 200, h 200, 200 up) while nobody
    // talks.
    if let Some(t) = talk
        && t.active.is_none()
        && !game.talking
    {
        shapes.push(rc::Shape {
            owner: TALK,
            group: 0,
            command: TALK_COMMAND,
            centre: (t.position + Vec3::Y * REACTION_SPHERE[2]).to_array(),
            radius: REACTION_SPHERE[0],
            height: REACTION_SPHERE[1],
            owner_yaw: t.yaw,
            attack: None,
        });
    }
    let p = Vec3::from_array(game.player.position);
    let drive = game.drive.as_ref();
    let battle = game.enemy.is_some() || second_deck::battle(game);
    let sora = rc::Sora {
        centre: (p + Vec3::Y * SORA_OBJ[2]).to_array(),
        radius: SORA_OBJ[0],
        height: SORA_OBJ[1],
        yaw: game.player.yaw,
        battle,
        // [U] Not while a summon's limit scene captures him (see
        // `sora_actor`).
        no_control: !summon::in_limit_scene(game) && game.player.no_control(&game.clips),
        state: sora_actor(game).state,
        form: drive.filter(|d| d.active).map_or(0, |d| d.form),
        // FUN_1403b6a10 refuses everything for +0xde0 == 0xc, a state the
        // room's forms never take [U: which]; Antiform (6) only dims the
        // slot (FUN_140408520, below).
        anti: false,
        class: 1,
        limit: drive.is_some_and(|d| d.limit.is_some()),
        won: game.battle_won,
        leader: true,
        hold: state.hold,
        no_shapes: game.party_limit.is_some(),
        ..Default::default()
    };
    let sora_target = rc::Target {
        owner: SORA,
        ..Default::default()
    };
    // The lock-on target: the actor whose TARGET point is the one held.
    let locked = game
        .selected_target
        .filter(|_| game.target_locked)
        .and_then(|at| {
            actors.iter().find_map(|a| {
                let t = a.battle.as_ref()?.pose.targets.first()?;
                (t.centre.distance(Vec3::from_array(at)) < 1.).then_some(a.id as u32)
            })
        });
    // The manager's record 1: whichever of a party limit, a summon or a
    // Limit Form limit offered last tick.
    let offered = game
        .party_limit
        .as_ref()
        .and_then(|l| l.offers[1])
        .map(|c| (c, sora_target))
        .or_else(|| {
            summon::offers(game).and_then(|o| o[1]).map(|c| {
                (
                    c,
                    rc::Target {
                        owner: SUMMON,
                        ..Default::default()
                    },
                )
            })
        })
        .or_else(|| drive.and_then(|d| d.offer).map(|c| (c, sora_target)));
    rc::Inputs {
        sora,
        sora_target,
        shapes,
        attacks,
        offered,
        shown: state.shown().map(|c| (c.command, c.target)),
        locked,
        auto: auto(game, menu_kind),
    }
}

/// The auto tail's inputs (FUN_1404081c0 after the shapes).
fn auto(game: &Game, menu_kind: Option<u8>) -> rc::Auto {
    let hp = game
        .enemy
        .as_ref()
        .map(|rt| (rt.sora_hp, rt.sora_target.max_hp));
    let members: Vec<_> = game.party.iter().filter(|m| !m.stored).collect();
    let sora = Vec3::from_array(game.player.position);
    // FUN_1403e7950: the limits the deck lists usable whose members carry
    // Auto Limit and stand within 500 of Sora. The room's members carry no
    // ability record: their Auto Limit counts as equipped [test-room choice,
    // as the Limit list's].
    let limits = game
        .second_deck
        .as_ref()
        .map(|sd| {
            let room = second_deck::room(game, None);
            sd.limit_list(&room)
                .into_iter()
                .filter(|&(_, colour)| colour < 4)
                .filter_map(|(cmd, _)| {
                    let row = sd.limits.iter().find(|l| l.command == cmd)?;
                    let member = members.iter().position(|m| {
                        m.kind.character == row.members[1]
                            && Vec3::from_array(m.follower.position).distance(sora)
                                <= rc::AUTO_LIMIT_DISTANCE
                    })?;
                    Some((
                        cmd,
                        rc::Target {
                            owner: member as u32,
                            ..Default::default()
                        },
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    rc::Auto {
        menu_kind,
        blocked: false,
        status: hp.is_some(),
        auto_summon: game.player.abilities.has(56),
        member_down: members.iter().any(|m| m.hp <= 0),
        hp_low: hp.is_some_and(|(cur, max)| cur <= max / 4),
        // FUN_1403e75d0: Sora up, no party limit, in battle, no MP charge.
        limit_ready: game.party_limit.is_none()
            && second_deck::battle(game)
            && !game.player.mp_charging
            && hp.is_some_and(|(cur, _)| cur > 0),
        limits,
    }
}

/// FUN_140401d60's cases that read the room: the Drive command of a form
/// (case 4, the Drive submenu's colour), the summons (0x48) and the limits
/// (0x2c3) as the second deck colours them.
fn exec_modes(game: &Game, commands: &Commands) -> HashMap<u16, u8> {
    let mut modes = HashMap::new();
    if let Some(d) = game.drive.as_ref() {
        for (&id, row) in &commands.rows {
            if row.exec == 4 && (1..6).contains(&row.argument) {
                let ok = d.selectable(row.argument as u8, &game.player.drive);
                modes.insert(id, if ok { 0 } else { rc::MODE_DIM });
            }
        }
    }
    if let Some(sd) = game.second_deck.as_ref() {
        let room = second_deck::room(game, None);
        for (cmd, colour) in sd
            .summon_list(&room)
            .into_iter()
            .chain(sd.limit_list(&room))
        {
            modes.insert(cmd, colour.min(9) as u8);
        }
    }
    modes
}

/// The tours' Triangle presses, each on its own source's offer as before.
fn tour_press(game: &Game, shown: Option<rc::Candidate>, tick: u64) -> bool {
    let Some(c) = shown else { return false };
    match c.source {
        rc::Source::Offered if game.party_limit.is_some() => {
            game.deck_tour && deck_tour::reaction(tick)
                || game.limit_tour && limit_tour::reaction(tick)
        }
        rc::Source::Offered if c.target.owner == SUMMON => {
            game.summon_tour && summon_tour::reaction(tick)
        }
        rc::Source::Offered => game.autoplay.is_some(),
        rc::Source::Shape if c.target.owner == TALK => {
            game.autoplay.is_some() && AUTOPLAY_TALK.contains(&tick)
        }
        rc::Source::Shape => {
            game.sephiroth_tour
                && super::sephiroth::tour_reaction(
                    tick,
                    game.player.action.airborne(),
                    game.offers_seen,
                )
        }
        _ => false,
    }
}

/// The request's execution (FUN_1403d6fc0's default branch for a reaction
/// row). Talk and the record-1 follow-ups are carried out by their modules
/// from [`State::press`].
fn carry_out(game: &mut Game, actors: &mut [GalleryActor], c: rc::Candidate, tick: u64) {
    match c.source {
        rc::Source::Shape if c.target.owner < SORA => {
            // Sora's update (FUN_1403a89a0) runs the request the same tick
            // (FUN_1403d6fc0 → vt+0x138 → FUN_1403a8b10); the press already
            // tested his windows (FUN_1403b6950). FUN_1401b15f0, which takes
            // a request only in a command window, is the party members'
            // update (+0xe18 flags), not his.
            if let Some(battle) = actors
                .iter_mut()
                .find(|a| a.id as u32 == c.target.owner)
                .and_then(|a| a.battle.as_mut())
            {
                // FUN_1403b5fa0: trigger 11 on the target's script.
                battle.brain.command(c.command);
                let action = format!("{:?}", game.player.action);
                let frame = game.player.frame;
                game.event(serde_json::json!({"event":"reaction_executed","tick":tick,
                    "command":c.command,"owner":c.target.owner,"sora_action":action,"sora_frame":frame}));
            }
        }
        rc::Source::Auto(item) => {
            let Some(row) = game
                .reaction_commands
                .as_ref()
                .and_then(|r| r.rows.get(&c.command))
                .copied()
            else {
                return;
            };
            match row.exec {
                // An auto form: the Drive command of its form.
                4 => {
                    if let Some(d) = game.drive.as_mut() {
                        d.request = Some(row.argument as u8);
                    }
                }
                // Auto Summon: the summon's command.
                0x48 => {
                    if let Err(e) = summon::start(game, c.command, tick) {
                        game.event(serde_json::json!({"event":"auto_summon_refused",
                            "tick":tick,"command":c.command,"error":e}));
                    }
                }
                // Auto Limit: the limit's command.
                _ => game.limit_request = Some(c.command),
            }
            game.event(serde_json::json!({"event":"reaction_auto","tick":tick,
                "item":item,"command":c.command}));
        }
        _ => {}
    }
}

/// The slot's marker for the HUD: its creation serial, `fiel` group and the
/// target point (Sora's body for his own offers).
pub fn marker(game: &Game) -> Option<(u32, usize, [f32; 3])> {
    let (serial, group) = game.reaction.slot.marker_group()?;
    let target = game.reaction.slot.shown.target;
    let at = game
        .reaction
        .points
        .iter()
        .find(|(t, _)| *t == target)
        .map(|(_, at)| *at)
        .unwrap_or_else(|| {
            let p = Vec3::from_array(game.player.position);
            (p + Vec3::Y * SORA_OBJ[2]).to_array()
        });
    Some((serial, usize::from(group), at))
}

/// The slot as the command menu shows it (FUN_14015d670's inputs).
pub fn menu_reaction(game: &Game) -> Option<field_command_menu::Reaction> {
    // The room hides the slot while a field talk runs.
    if game.talking {
        return None;
    }
    let c = game.reaction.shown()?;
    let commands = game.reaction_commands.as_ref()?;
    let row = commands.rows.get(&c.command)?;
    Some(field_command_menu::Reaction {
        command: c.command,
        label: commands
            .names
            .get(&c.command)
            .or((c.target.owner == TALK).then_some(&game.reaction.talk_label))
            .cloned()
            .unwrap_or_default(),
        mode: c.mode,
        flag3: row.flags & 8 != 0,
        restart: c.flags & 2 != 0,
    })
}
