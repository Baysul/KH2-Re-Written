//! The unattended `--limit-form-tour <dir>` (in the magic tour's setup:
//! Sora still, the Shadow frozen nearby): Sora drives into Limit Form, plays
//! a ground combo on the Shadow (its finisher is the form's PTYA record 3,
//! Zantetsuken), then runs its four limits in the order of its Limit list,
//! each follow-up taken as soon as it is offered. Before each limit his MP is full and he
//! stands in front of the Shadow, facing it [test-room values: as a player
//! would walk up with MP to spare].
use player::limit_form::LIMITS;

/// The combo's presses, once the form change has played out.
const COMBO: std::ops::Range<u64> = 250..340;
/// The first limit.
const FIRST: u64 = 400;
/// Ticks each limit gets.
const STEP: u64 = 500;
/// How far in front of the Shadow Sora stands before each limit.
pub const STAND_OFF: f32 = 160.;

/// The limit to request this tick.
pub fn limit(tick: u64) -> Option<u16> {
    let t = tick.checked_sub(FIRST)?;
    if !t.is_multiple_of(STEP) {
        return None;
    }
    LIMITS.get(usize::try_from(t / STEP).ok()?).copied()
}

/// The tick before each limit: MP topped up, Sora placed.
pub fn refill(tick: u64) -> bool {
    limit(tick + 5).is_some()
}

/// Sora is placed before the combo too.
pub fn place(tick: u64) -> bool {
    refill(tick) || tick + 5 == COMBO.start
}

/// The Attack presses of the combo.
pub fn attack(tick: u64) -> bool {
    COMBO.contains(&tick) && tick.is_multiple_of(10)
}

pub fn captures() -> Vec<u64> {
    let mut c = vec![COMBO.start + 40, COMBO.end + 10, FIRST - 20];
    for k in 0..LIMITS.len() as u64 {
        let s = FIRST + k * STEP;
        c.extend([s + 30, s + 90, s + 150, s + 240, s + 360]);
    }
    c
}

pub fn end() -> u64 {
    FIRST + LIMITS.len() as u64 * STEP + 20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_limit_in_turn() {
        assert_eq!(limit(FIRST), Some(LIMITS[0]));
        assert_eq!(limit(FIRST + STEP), Some(LIMITS[1]));
        assert_eq!(limit(FIRST + 3 * STEP), Some(LIMITS[3]));
        assert_eq!(limit(FIRST + 4 * STEP), None);
        assert_eq!(limit(FIRST + 1), None);
        assert!(refill(FIRST - 5) && !refill(FIRST));
        assert!(place(COMBO.start - 5) && attack(COMBO.start) && !attack(FIRST));
        assert!(captures().iter().all(|&c| c < end()));
    }
}
