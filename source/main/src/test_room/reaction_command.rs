//! Reaction commands the battle actors offer through their own collision:
//! each tick the Triangle slot's search (`player::reaction_command`, the
//! command manager's FUN_1404081c0 path) over the REACTION shapes of the
//! script-driven actors, and the press that runs the winner (FUN_1403b2340
//! takes Triangle; FUN_1401b15f0 consumes the request in an actionable
//! window; FUN_1403a8b10's default branch hands it to the target's script as
//! trigger 11, FUN_1403b5fa0).
use super::*;
use enemy_gallery::GalleryActor;
use player::reaction_command as rc;

/// The command records and names the search reads (03system `cmd`).
pub struct Commands {
    pub rows: HashMap<u16, rc::Row>,
    pub names: HashMap<u16, Vec<u8>>,
}

/// The records of 03system `cmd`, and the names (messages) of the commands
/// that can be offered in battle (flag bit 4).
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
                    flags: c.flags,
                    facing_cos: c.reaction.facing_cos,
                    target_yaw: c.reaction.target_yaw,
                    target_cos: c.reaction.target_cos,
                    priority: c.reaction.priority,
                    chain: c.chain,
                    distance: c.reaction.distance,
                    form_mask: c.form_mask,
                },
            )
        })
        .collect();
    let offered: Vec<&data::command_table::Command> = table
        .iter()
        .filter(|c| c.flags & 0x10 != 0 && c.name != 0)
        .collect();
    let ids: Vec<u32> = offered.iter().map(|c| c.name).collect();
    let texts = names(&ids)?;
    let names = offered
        .iter()
        .zip(texts)
        .filter_map(|(c, t)| Some((c.id, t?)))
        .collect();
    Ok(Commands { rows, names })
}

/// This tick's offer: the command, the actor that offers it and whether
/// it shows dimmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Current {
    pub offer: rc::Offer,
}

/// The search over the actors' REACTION shapes (their poses of the last
/// update), then Triangle. A press on a dimmed command plays the refusal
/// sound (FUN_140402110 -> se000 #5); one Sora cannot take now is dropped,
/// as the request is cleared every tick.
pub fn tick(game: &mut Game, actors: &mut [GalleryActor], tick: u64) {
    let Some(commands) = game.reaction_commands.as_ref() else {
        game.enemy_reaction = None;
        return;
    };
    let mut shapes = Vec::new();
    for actor in actors.iter() {
        let Some(battle) = actor.battle.as_ref() else {
            continue;
        };
        if battle.error.is_some() || !battle.brain.reaction_active() {
            continue;
        }
        for &(ref shape, kind, argument) in &battle.pose.reactions {
            // Kind 14 offers the command a clashed attack left (+0xcf0) and
            // argument 0x83 the spawn's: [U] neither is kept here.
            if kind != data::object_collision::REACTION || argument == 0x83 {
                continue;
            }
            let Ok(command) = u16::try_from(argument) else {
                continue;
            };
            shapes.push(rc::Shape {
                owner: actor.id as u32,
                group: shape.group,
                command,
                centre: shape.centre.to_array(),
                radius: shape.radius,
                height: shape.height,
                owner_yaw: actor.yaw,
            });
        }
    }
    let p = Vec3::from_array(game.player.position);
    let sora = rc::Sora {
        centre: (p + Vec3::Y * SORA_OBJ[2]).to_array(),
        radius: SORA_OBJ[0],
        height: SORA_OBJ[1],
        yaw: game.player.yaw,
        battle: game.enemy.is_some(),
        no_control: game.player.no_control(&game.clips),
        state: None,
        form: game
            .drive
            .as_ref()
            .filter(|d| d.active)
            .map_or(0, |d| d.form),
        anti: game
            .drive
            .as_ref()
            .is_some_and(|d| d.active && d.form == player::forms::ANTI),
        class: 1,
        limit: game.party_limit.is_some(),
        won: game.battle_won,
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
    let shown = game.enemy_reaction.map(|c| c.offer.owner);
    let offer = rc::search(
        &shapes,
        &sora,
        |c| commands.rows.get(&c).copied(),
        |s| {
            actors
                .iter()
                .find(|a| a.id as u32 == s.owner)
                .and_then(|a| a.battle.as_ref())
                .is_some_and(|b| {
                    b.brain.reaction_offered(
                        i32::from(s.command),
                        i32::from(s.group),
                        b.assets.default_groups,
                    )
                })
        },
        shown,
        locked,
    );
    if offer.map(|o| o.command) != game.enemy_reaction.map(|c| c.offer.command) {
        if offer.is_some() {
            game.offers_seen += 1;
        }
        game.event(serde_json::json!({"event":"reaction_offer","tick":tick,
            "command":offer.map(|o| o.command),"owner":offer.map(|o| o.owner),
            "dimmed":offer.map(|o| o.dimmed())}));
    }
    game.enemy_reaction = offer.map(|offer| Current { offer });
    let Some(offer) = offer else {
        return;
    };
    let pressed = std::mem::take(&mut game.rc_queued)
        || game.sephiroth_tour
            && super::sephiroth::tour_reaction(
                tick,
                game.player.action.airborne(),
                game.offers_seen,
            );
    if !pressed {
        return;
    }
    if offer.dimmed() {
        game.sound_queue.push((5, "menu_denied"));
        return;
    }
    if !game.player.can_command(&game.clips) {
        game.event(serde_json::json!({"event":"reaction_dropped","tick":tick,
            "command":offer.command}));
        return;
    }
    if let Some(battle) = actors
        .iter_mut()
        .find(|a| a.id as u32 == offer.owner)
        .and_then(|a| a.battle.as_mut())
    {
        battle.brain.command(offer.command);
        let action = format!("{:?}", game.player.action);
        let frame = game.player.frame;
        game.event(serde_json::json!({"event":"reaction_executed","tick":tick,
            "command":offer.command,"owner":offer.owner,"sora_action":action,"sora_frame":frame}));
    }
}
