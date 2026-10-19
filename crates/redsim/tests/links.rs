//! Pins the link table (`redsim::links`) on a few entries the doors rely on.

use redsim::links::{GADGETS, response};

fn g(name: &str) -> &'static redsim::links::Gadget {
    GADGETS.iter().find(|g| g.name == name).unwrap()
}

#[test]
fn observer_busy_window_merges_a_short_pulse() {
    // the lever's on and off 1 gt apart: one observer pulse, not two
    assert_eq!(response(g("observer on the lever"), &[0, 1], 40), vec![(2, 4)]);
    // 4 gt apart: two pulses
    assert_eq!(response(g("observer on the lever"), &[0, 4], 40), vec![(2, 4), (6, 8)]);
}

#[test]
fn lamp_lights_at_once_and_falls_4_gt_after_the_unpower() {
    assert_eq!(response(g("lamp"), &[0, 1], 40), vec![(1, 5)]);
    // two pulses 2 gt apart merge into one window
    assert_eq!(response(g("lamp"), &[0, 2, 4, 6], 40), vec![(1, 10)]);
    // an observer watching the lamp fires twice per window
    assert_eq!(response(g("observer on a lamp"), &[0, 1], 40), vec![(2, 4), (7, 9)]);
}

#[test]
fn bulb_flip_flop_turns_two_pulses_into_one_window() {
    assert_eq!(response(g("observer -> bulb (T flip-flop)"), &[0, 2, 10, 12], 40), vec![(2, 12)]);
}

#[test]
fn comparator_swallows_a_1_gt_pulse_and_repeaters_stretch_it() {
    assert_eq!(response(g("comparator"), &[0, 1], 40), vec![]);
    assert_eq!(response(g("repeater d2"), &[0, 1], 40), vec![(4, 8)]);
}
