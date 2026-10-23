//! Sora's reaction-command search, translated from the executable (Steam
//! build 15194255): the per-tick driver FUN_1404081c0, the collision path
//! FUN_140407de0, the score FUN_1404088f0, the candidate test FUN_140408080
//! with the form gate FUN_1403b6a10 and the mode FUN_140401d60.
//!
//! Every tick the command manager looks for the command to show in the
//! Triangle slot. Objects offer commands through REACTION shapes of their
//! collision (whether a shape's group is on is the object's business; see the
//! enemy crate's `reaction` module); a candidate beats the current best by a
//! higher priority, or by a lower score at the same priority. The score is the
//! horizontal distance, shortened for the target already shown and more for
//! the locked-on one; bit 30 of the score marks a command that may not be used
//! now (drawn dimmed).

/// Bit 30 of a score: the candidate is shown dimmed.
pub const DIMMED: i32 = 0x4000_0000;

/// A command record's fields the search reads (03system `cmd`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Row {
    pub id: u16,
    /// +0xc.
    pub flags: u32,
    /// +0x10, +0x14, +0x18: Sora's facing gate, the target's yaw offset and
    /// the target's facing gate (cosines; -1 = none).
    pub facing_cos: f32,
    pub target_yaw: f32,
    pub target_cos: f32,
    /// +0x1e.
    pub priority: u8,
    /// +0x22: the state id a state-locked command needs (Sora +0x638).
    pub chain: u16,
    /// +0x28: past this horizontal distance the offer is dimmed (0 none).
    pub distance: u16,
    /// +0x2c: forms that cannot use the command (bit f).
    pub form_mask: u16,
}

/// Sora's side of the search.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sora {
    /// The centre, radius and height of his OBJ shape (collision kind 1).
    pub centre: [f32; 3],
    pub radius: f32,
    pub height: f32,
    /// His heading about Y (forward is (sin, 0, cos)).
    pub yaw: f32,
    /// The battle flag (DAT_142a11404 != 0).
    pub battle: bool,
    /// A no-control motion or state (+0x18c & 0x1000020, +0x120 bit 23).
    pub no_control: bool,
    /// Motion range 0x1a's state (+0x18c bit 21) and its value (+0x638).
    pub state: Option<u16>,
    /// The Drive form in effect (bit `form` of a record's mask disables
    /// it, FUN_1403b6cb0; 0 without a form).
    pub form: u8,
    /// Anti form (+0xde0 == 0x0c): no reaction command at all.
    pub anti: bool,
    /// The class byte of his model's object entry (+7): 1 Sora, 0xe Roxas,
    /// 0x14 the lion, 0x1b Mickey.
    pub class: u8,
    /// A limit runs (+0x6cb bit 0 with +0xe14).
    pub limit: bool,
    /// DAT_142a11400 bit 1: the battle is won (commands are locked).
    pub won: bool,
}

/// One REACTION shape in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    /// Who offers it (the host's object key) and the shape's group.
    pub owner: u32,
    pub group: u8,
    pub command: u16,
    pub centre: [f32; 3],
    pub radius: f32,
    pub height: f32,
    /// The owner's heading about Y.
    pub owner_yaw: f32,
}

/// The search's result: the command for the Triangle slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    pub command: u16,
    pub owner: u32,
    pub group: u8,
    pub priority: u8,
    pub score: i32,
    /// The mode byte (FUN_140401d60; 5 when the score is dimmed): modes 4
    /// and above draw at 0.6 brightness.
    pub mode: u8,
}

impl Offer {
    pub fn dimmed(&self) -> bool {
        self.mode >= 4
    }
}

/// FUN_1403c94c0 with FUN_1403c9890: a sphere against an ellipsoid, both
/// given as centre, radius and height (half the vertical extent).
pub fn shapes_touch(a: [f32; 3], ra: f32, ha: f32, b: [f32; 3], rb: f32, hb: f32) -> bool {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    let (r, h) = (ra + rb, ha + hb);
    if r <= 0. || h <= 0. {
        return false;
    }
    (dx * dx + dz * dz) / (r * r) + dy * dy / (h * h) < 1.
}

/// FUN_1403b6a10: whether Sora's model may use the command at all.
pub fn form_allows(row: &Row, sora: &Sora) -> bool {
    if sora.anti {
        return false;
    }
    if sora.limit {
        return row.flags & 0x8000 == 0;
    }
    match sora.class {
        1 => row.flags & 0x400 == 0,
        0xe => row.flags & 0x800 == 0,
        0x14 => row.flags & 0x1000 == 0,
        0x1b => row.flags & 0x2000 == 0,
        _ => true,
    }
}

/// FUN_140401d60's default path for a reaction row: 5 (dimmed) while the
/// command may not be used now (FUN_1403b6cf0: a no-control-gated command
/// while Sora has no control, a field-only command in battle, or any command
/// without flag bit 5 once the battle is won; FUN_1403b6cb0: the form's bit
/// of the row's mask), else 0. [U] The command-group lock FUN_1403f29c0
/// (mgr+0xbe4) is not kept.
pub fn mode(row: &Row, sora: &Sora) -> u8 {
    let locked = row.flags & 2 != 0 && sora.no_control
        || row.flags & 0x1_0000 != 0 && !sora.battle
        || row.flags & 0x20 == 0 && sora.won
        || u32::from(row.form_mask) >> sora.form & 1 != 0;
    if locked { 5 } else { 0 }
}

/// FUN_1404088f0: the horizontal distance (shortened by 0.8 for the target
/// already shown, by 0.5 for the locked-on one), with bit 30 set when a
/// facing gate fails or a no-control-gated command meets Sora without
/// control.
pub fn score(row: &Row, sora: &Sora, shape: &Shape, shown: bool, locked: bool) -> i32 {
    let d = horizontal(sora.centre, shape.centre);
    let mut s = d as i32;
    if locked {
        s = (s as f32 * 0.5) as i32;
    } else if shown {
        s = (s as f32 * 0.8) as i32;
    }
    s = s.min(0x3fff_ffff);
    let to = [
        shape.centre[0] - sora.centre[0],
        shape.centre[2] - sora.centre[2],
    ];
    let len = to[0].hypot(to[1]);
    let unit = if len > 0. {
        [to[0] / len, to[1] / len]
    } else {
        [0., 0.]
    };
    let facing = [sora.yaw.sin(), sora.yaw.cos()];
    let mut dim =
        row.facing_cos > -1. && unit[0] * facing[0] + unit[1] * facing[1] < row.facing_cos;
    if row.target_cos > -1. {
        // The target's heading turned by the row's offset plus pi: Sora must
        // stand inside its cone.
        let a = shape.owner_yaw + row.target_yaw + std::f32::consts::PI;
        dim |= unit[0] * a.sin() + unit[1] * a.cos() < row.target_cos;
    }
    dim |= row.flags & 2 != 0 && sora.no_control;
    if dim { s | DIMMED } else { s }
}

fn horizontal(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

/// FUN_1404081c0's collision part (FUN_140407de0 per shape, FUN_140408080
/// per candidate, FUN_140408bd0 storing the winner): the best offer among
/// the shapes, given the record of each command, which target is shown and
/// which is locked on. `offered` is the owner's enable test (FUN_1403ba600).
pub fn search(
    shapes: &[Shape],
    sora: &Sora,
    row_of: impl Fn(u16) -> Option<Row>,
    offered: impl Fn(&Shape) -> bool,
    shown: Option<u32>,
    locked: Option<u32>,
) -> Option<Offer> {
    let mut best: Option<Offer> = None;
    for shape in shapes {
        if shape.command == 0 || !offered(shape) {
            continue;
        }
        let Some(row) = row_of(shape.command) else {
            continue;
        };
        if sora.battle && row.flags & 0x10 == 0 {
            continue;
        }
        let state_ok = row.chain != 0 && sora.state == Some(row.chain);
        if row.flags & 0x40 != 0 && !state_ok {
            continue;
        }
        if !shapes_touch(
            shape.centre,
            shape.radius,
            shape.height,
            sora.centre,
            sora.radius,
            sora.height,
        ) {
            continue;
        }
        let mut priority = row.priority;
        if priority < 0xca && state_ok {
            priority = 0xca;
        }
        let s = score(
            &row,
            sora,
            shape,
            shown == Some(shape.owner),
            locked == Some(shape.owner),
        );
        if let Some(b) = best
            && (priority < b.priority || priority == b.priority && b.score <= s)
        {
            continue;
        }
        if !form_allows(&row, sora) {
            continue;
        }
        let mut s = s;
        if row.distance != 0 && horizontal(sora.centre, shape.centre) > f32::from(row.distance) {
            s |= DIMMED;
        }
        best = Some(Offer {
            command: row.id,
            owner: shape.owner,
            group: shape.group,
            priority,
            score: s,
            mode: mode(&row, sora),
        });
    }
    // FUN_1404081c0's tail: a dimmed score shows the command at mode 5.
    best.map(|mut b| {
        if b.score >= 0 && b.score & DIMMED != 0 {
            b.mode = 5;
        }
        b
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block() -> Row {
        Row {
            id: 664,
            flags: 0xb917,
            facing_cos: -1.,
            target_yaw: 0.,
            target_cos: -1.,
            priority: 100,
            chain: 0,
            distance: 0,
            form_mask: 0,
        }
    }

    fn sora() -> Sora {
        Sora {
            centre: [0., 85., 0.],
            radius: 35.,
            height: 55.,
            battle: true,
            class: 1,
            ..Default::default()
        }
    }

    fn sphere(owner: u32, at: [f32; 3]) -> Shape {
        Shape {
            owner,
            group: 3,
            command: 664,
            centre: at,
            radius: 3000.,
            height: 3000.,
            owner_yaw: 0.,
        }
    }

    #[test]
    fn a_wide_sphere_offers_its_command_in_battle_unless_its_group_is_off() {
        let s = [sphere(7, [0., 170., 750.])];
        let rows = |c: u16| (c == 664).then(block);
        let o = search(&s, &sora(), rows, |_| true, None, None).unwrap();
        assert_eq!((o.command, o.owner, o.score, o.mode), (664, 7, 750, 0));
        assert!(search(&s, &sora(), rows, |_| false, None, None).is_none());
        // Locked on: half the distance.
        let o = search(&s, &sora(), rows, |_| true, None, Some(7)).unwrap();
        assert_eq!(o.score, 375);
        // Out of the sphere.
        let far = [sphere(7, [0., 170., 3100.])];
        assert!(search(&far, &sora(), rows, |_| true, None, None).is_none());
    }

    #[test]
    fn no_control_dims_a_flag_bit_1_command_and_the_lion_never_sees_it() {
        let s = [sphere(7, [0., 170., 750.])];
        let rows = |c: u16| (c == 664).then(block);
        let mut stuck = sora();
        stuck.no_control = true;
        let o = search(&s, &stuck, rows, |_| true, None, None).unwrap();
        assert!(o.dimmed());
        assert_eq!(o.score & !DIMMED, 750);
        let mut lion = sora();
        lion.class = 0x14;
        assert!(search(&s, &lion, rows, |_| true, None, None).is_none());
    }

    #[test]
    fn higher_priority_wins_then_the_lower_score() {
        let mut talk = block();
        talk.id = 31;
        talk.priority = 120;
        let mut s = [sphere(1, [0., 170., 200.]), sphere(2, [0., 170., 900.])];
        s[1].command = 31;
        let rows = |c: u16| match c {
            664 => Some(block()),
            31 => Some(talk),
            _ => None,
        };
        assert_eq!(
            search(&s, &sora(), rows, |_| true, None, None)
                .unwrap()
                .command,
            31
        );
        s[1].command = 664;
        assert_eq!(
            search(&s, &sora(), rows, |_| true, None, None)
                .unwrap()
                .owner,
            1
        );
    }

    #[test]
    fn the_ellipsoid_test_uses_both_extents() {
        assert!(shapes_touch([0.; 3], 10., 10., [0., 0., 19.], 10., 1.));
        assert!(!shapes_touch([0.; 3], 10., 10., [0., 0., 20.], 10., 1.));
        assert!(!shapes_touch([0.; 3], 10., 10., [0., 11., 0.], 10., 1.));
    }
}
