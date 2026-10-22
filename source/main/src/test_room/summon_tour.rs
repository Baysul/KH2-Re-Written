//! The unattended `--summon-tour <dir>` (in the magic tour's setup: Sora
//! stands still, the Shadow is frozen nearby). Each summon of `sumn` in
//! turn is asked for as the Summon list's pick would, with a full Drive
//! gauge [tour setup]; the first three are dismissed from the second deck's
//! Dismiss row, the last runs out of time. Between the deck swaps Sora
//! attacks (Attack, the top row) so that the summons that answer Sora's
//! attacks (Peter Pan, Genie) have something to answer. The tour owns the
//! four charms.
use visual::field_command_menu::Buttons;

/// The first summon's tick and the ticks between summons.
const FIRST: u64 = 200;
const STEP: u64 = 1400;
/// Ticks after a call: the deck swap that shows Dismiss, Dismiss itself,
/// and the swap back.
const SHOW_DECK: u64 = 650;
const DISMISS: u64 = 900;
const HIDE_DECK: u64 = 1000;
/// The last summon's time (3 bars of 600) plus its leaving.
const LAST: u64 = 2600;
pub const SUMMONS: usize = 4;

/// The summon (row index) the tour asks for at `tick`.
pub fn request(tick: u64) -> Option<usize> {
    let k = tick.checked_sub(FIRST)?;
    (k.is_multiple_of(STEP) && k / STEP < SUMMONS as u64).then_some((k / STEP) as usize)
}

/// The tick a summon is dismissed (the last one is not).
pub fn dismiss(tick: u64) -> bool {
    tick.checked_sub(FIRST + DISMISS)
        .is_some_and(|k| k.is_multiple_of(STEP) && k / STEP < SUMMONS as u64 - 1)
}

/// Ticks after a call when Sora attacks, every `ATTACK_EVERY` ticks: after
/// the summon has come and before the deck swap, then after the swap back
/// (the last summon, never dismissed, to near its end).
const ATTACKS: [std::ops::Range<u64>; 2] = [300..600, 1050..1350];
const LAST_ATTACKS: std::ops::Range<u64> = 1050..2400;
const ATTACK_EVERY: u64 = 20;

fn attack(tick: u64) -> bool {
    let Some(k) = tick.checked_sub(FIRST) else {
        return false;
    };
    let n = (k / STEP).min(SUMMONS as u64 - 1);
    let t = k - n * STEP;
    let last = n == SUMMONS as u64 - 1;
    t.is_multiple_of(ATTACK_EVERY)
        && (ATTACKS[0].contains(&t)
            || (!last && ATTACKS[1].contains(&t))
            || (last && LAST_ATTACKS.contains(&t)))
}

/// Left on the second deck around the first Dismiss; Attack between.
pub fn press(tick: u64) -> Buttons {
    Buttons {
        left: tick == FIRST + SHOW_DECK || tick == FIRST + HIDE_DECK,
        decide: attack(tick),
        ..Buttons::default()
    }
}

pub fn captures() -> Vec<u64> {
    let mut c = Vec::new();
    for k in 0..SUMMONS as u64 {
        let s = FIRST + k * STEP;
        c.extend([
            s + 20,
            s + 140,
            s + 200,
            s + 400,
            s + 700,
            s + 960,
            s + 1150,
        ]);
    }
    let last = FIRST + (SUMMONS as u64 - 1) * STEP;
    c.extend([last + 1900, last + 2100]);
    c.sort_unstable();
    c.dedup();
    c
}

pub fn end() -> u64 {
    FIRST + (SUMMONS as u64 - 1) * STEP + LAST
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_summon_is_asked_once_and_all_but_the_last_dismissed() {
        let asked: Vec<_> = (0..end()).filter_map(request).collect();
        assert_eq!(asked, vec![0, 1, 2, 3]);
        assert_eq!((0..end()).filter(|&t| dismiss(t)).count(), SUMMONS - 1);
        assert!(captures().iter().all(|&c| c < end()));
        assert!(press(FIRST + SHOW_DECK).left && !press(FIRST).left);
        // No Attack near a deck swap or a Dismiss.
        for k in 0..SUMMONS as u64 {
            let s = FIRST + k * STEP;
            for t in [SHOW_DECK, DISMISS, HIDE_DECK] {
                assert!((s + t - 30..s + t + 30).all(|x| !press(x).decide));
            }
        }
        assert!(press(FIRST + 300).decide && press(FIRST + 3 * STEP + 2000).decide);
    }
}
