use super::*;

#[test]
fn ticks_are_readable() {
    assert_eq!(tick_step(2.0), 0.2);
    assert_eq!(tick_step(20.0), 2.0);
    assert_eq!(tick_step(600.0), 60.0);
}

#[test]
fn wave_path_is_closed() {
    let d = wave_path(&[(0.0, 0.5), (-0.5, 0.0)]);
    assert!(d.starts_with("M0,") && d.ends_with('Z'));
}
