//! Synthetic attacks only.
use super::*;

#[test]
fn team_three_counts_as_four_and_the_mask_excludes_own_team_and_neutral() {
    let mut a = Attack::new(None, 7, 0, 0, 3);
    assert_eq!(a.team, 4);
    assert_eq!(a.team_mask, !(1u8 << 4 | 1));
    a.set_team(1);
    assert_eq!(a.team_mask, !(1u8 << 1 | 1));
    a.set_team(9);
    assert_eq!(
        a.team_mask, !1u8,
        "a team past the mask byte only drops bit 0"
    );
}

#[test]
fn a_second_radius_of_zero_repeats_the_first() {
    let mut a = Attack::new(None, 0, 0, 0, 0);
    a.set_radius(12., 0.);
    assert_eq!(a.radius, (12., 12.));
    a.set_radius(12., 30.);
    assert_eq!(a.radius, (12., 30.));
}

#[test]
fn moving_far_sets_the_far_flag_and_keeps_the_previous_position() {
    let mut a = Attack::new(None, 0, 0, 0, 0);
    a.set_position([0., 0., 0., 1.], 1.);
    assert_eq!(
        a.previous,
        [0., 0., 0., 1.],
        "first placement has no history"
    );
    a.set_position([1., 0., 0., 1.], 1.);
    assert_eq!(a.flags & flag::MOVED_FAR, 0);
    assert_eq!(a.previous, [0., 0., 0., 1.]);
    a.set_position([5., 0., 0., 1.], 1.);
    assert_ne!(a.flags & flag::MOVED_FAR, 0);
    assert_eq!(a.previous, [1., 0., 0., 1.]);
    assert_ne!(a.flags & flag::HAS_POSITION, 0);
}

#[test]
fn a_line_keeps_both_ends_and_a_placement_clears_the_line_flag() {
    let mut a = Attack::new(None, 0, 0, 0, 0);
    a.set_line([1., 2., 3., 1.], [4., 5., 6., 1.]);
    assert_eq!(a.position, [1., 2., 3., 1.]);
    assert_eq!(a.previous, [4., 5., 6., 1.]);
    assert_ne!(a.flags & flag::LINE, 0);
    a.set_position([0.; 4], 1.);
    assert_eq!(a.flags & flag::LINE, 0);
}

#[test]
fn hit_means_a_strike_the_map_or_a_guard() {
    let mut a = Attack::new(None, 0, 0, 0, 0);
    assert!(!a.is_hit());
    for bit in [
        flag::STRUCK,
        flag::HIT_WALL,
        flag::HIT_FLOOR,
        flag::REFLECTED,
    ] {
        let mut b = a.clone();
        b.mark_hit(bit);
        assert!(b.is_hit());
    }
    a.mark_hit(flag::MOVED_FAR);
    assert!(!a.is_hit());
}

#[test]
fn a_timed_shape_keeps_the_attack_radius_for_unset_ends() {
    let mut a = Attack::new(None, 0, 0, 0, 0);
    a.set_radius(10., 20.);
    a.set_time(30., 0., 50.);
    assert_eq!(a.time, [0., 30., 10., 50.]);
}
