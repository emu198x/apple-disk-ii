use emu198x_apple_disk_ii::{Disk, DiskError, DiskII, Track};
const HZ: u32 = 14_318_180;
fn card() -> DiskII {
    DiskII::new([0x6a; 256], [8; 256], HZ).expect("clock")
}
fn disk(byte: u8, protected: bool) -> Disk {
    Disk::new(
        [0; 160],
        vec![Track::new(vec![byte; 6400], 51200).expect("track")],
        32,
        protected,
    )
    .expect("disk")
}
fn advance(c: &mut DiskII, n: u32) {
    for t in 0..n {
        c.tick(t % 7 == 0);
    }
}
#[test]
fn soft_switches_drive_both_mechanisms_and_peeks_never_clock() {
    let mut c = card();
    assert_eq!(c.rom(255), 0x6a);
    c.insert(0, disk(0xff, true)).expect("drive");
    c.insert(1, disk(0x99, true)).expect("drive");
    let before = c.status();
    for _ in 0..100 {
        assert_eq!(c.data(12), Some(0));
        assert_eq!(c.data(13), None);
    }
    assert_eq!(before, c.status());
    c.select(1, None);
    c.select(3, Some(1));
    c.select(5, None);
    c.select(7, Some(1));
    assert_eq!(c.status().phases, 15);
    for s in [0, 2, 4, 6] {
        c.select(s, None);
    }
    assert_eq!(c.status().phases, 0);
    c.select(9, None);
    advance(&mut c, 10000);
    assert!(c.status().bit_reads[0] > 0);
    assert_eq!(c.status().bit_reads[1], 0);
    c.select(11, None);
    let position = c.status().bit_positions[0];
    advance(&mut c, 10000);
    assert_eq!(c.status().bit_positions[0], position);
    assert!(c.status().bit_reads[1] > 0);
    for (s, q6, q7) in [
        (13, true, false),
        (15, true, true),
        (12, false, true),
        (14, false, false),
    ] {
        c.select(s, Some(0xa5));
        assert_eq!((c.status().q6, c.status().q7), (q6, q7));
    }
    c.select(10, None);
    assert_eq!(c.status().selected_drive, 0);
    assert!(c.eject(0).expect("drive").is_some());
    assert!(c.disk(0).is_none());
    assert!(c.eject(0).expect("drive").is_none());
    assert!(matches!(c.eject(2), Err(DiskError::Drive)));
    assert!(matches!(c.insert(2, disk(0, true)), Err(DiskError::Drive)));
    assert!(c.disk(2).is_none());
}
#[test]
fn delayed_motor_off_is_one_second_and_repeated_off_does_not_extend_it() {
    let mut c = card();
    c.select(9, None);
    c.select(8, None);
    advance(&mut c, HZ / 2);
    c.select(8, None);
    advance(&mut c, HZ - HZ / 2 - 1);
    assert!(c.status().motor_on);
    c.tick(false);
    assert!(!c.status().motor_on);
    c.select(9, None);
    c.select(8, None);
    advance(&mut c, HZ / 2);
    c.select(9, None);
    advance(&mut c, HZ);
    assert!(c.status().motor_on);
    c.reset();
    assert!(!c.status().motor_on);
}
#[test]
fn stepping_hits_limits_and_preserves_relative_track_position() {
    let mut tracks = Vec::new();
    for length in [51200, 25600] {
        tracks.push(Track::new(vec![255; length / 8], length as u32).expect("track"));
    }
    let mut map = [0; 160];
    map[4..].fill(1);
    let mut c = card();
    c.insert(0, Disk::new(map, tracks, 32, true).expect("media"))
        .expect("drive");
    c.select(9, None);
    advance(&mut c, 10000);
    let old = c.status().bit_positions[0];
    // Phase 0 -> phase 1 -> phase 2 moves one track.
    for s in [1, 3, 0, 5, 2] {
        c.select(s, None);
    }
    assert_eq!(c.status().quarter_tracks[0], 4);
    assert_eq!(c.status().bit_positions[0], old / 2);
    // Opposite excitation does not move the head; invalid/all phases hold it.
    c.select(4, None);
    c.select(1, None);
    assert_eq!(c.status().quarter_tracks[0], 4);
    c.select(3, None);
    assert_eq!(
        c.status().quarter_tracks[0],
        1,
        "adjacent two-phase equilibrium"
    );
    c.select(5, None);
    c.select(7, None);
    assert_eq!(c.status().quarter_tracks[0], 1);
    c.reset();
    assert_eq!(c.status().quarter_tracks[0], 1);
    for _ in 0..100 {
        for s in [7, 0, 5, 6, 3, 4, 1, 2] {
            c.select(s, None);
        }
    }
    assert_eq!(
        c.status().quarter_tracks[0],
        1,
        "stopped motor does not seek"
    );
    c.select(9, None);
    for _ in 0..100 {
        for s in [7, 0, 5, 6, 3, 4, 1, 2] {
            c.select(s, None);
        }
    }
    assert_eq!(c.status().quarter_tracks[0], 0);
    for _ in 0..100 {
        for s in [3, 0, 5, 2, 7, 4, 1, 6] {
            c.select(s, None);
        }
    }
    assert_eq!(c.status().quarter_tracks[0], 136);
}
#[test]
fn write_gate_changes_work_media_but_not_protected_source() {
    for protected in [false, true] {
        let source = disk(255, protected);
        let mut work = source.clone();
        work.set_writable(true);
        assert!(source.write_protected());
        assert_eq!(work.write_protected(), protected);
        let mut c = card();
        c.insert(0, work).expect("drive");
        c.select(9, None);
        c.select(15, Some(0));
        advance(&mut c, 10000);
        c.select(14, None);
        c.tick(false);
        let bytes = c.disk(0).expect("media").tracks()[0].bytes();
        assert_eq!(bytes.iter().any(|&b| b != 255), !protected);
        assert!(source.tracks()[0].bytes().iter().all(|&b| b == 255));
        assert_eq!(c.status().bit_writes[0] > 0, !protected);
    }
}
#[test]
fn malformed_caller_inputs_fail_before_mounting() {
    assert_eq!(DiskError::Map.to_string(), "Disk II: Map");
    assert!(matches!(
        DiskII::new([0; 256], [0; 256], 1),
        Err(DiskError::Clock)
    ));
    assert_eq!(Track::new(vec![0], 0), Err(DiskError::Track));
    assert_eq!(Track::new(vec![0], 9), Err(DiskError::Track));
    assert!(matches!(
        Disk::new([0; 160], vec![], 32, false),
        Err(DiskError::Map)
    ));
    assert!(matches!(
        Disk::new([255; 160], vec![], 0, false),
        Err(DiskError::Timing)
    ));
}

#[test]
fn sequencer_write_transition_lands_on_the_surface_and_phase_one_blocks_it() {
    for phases in [false, true] {
        let mut c = DiskII::new([0; 256], [0x88; 256], HZ).expect("clock");
        let mut work = disk(0, false);
        work.set_writable(true);
        c.insert(0, work).expect("drive");
        c.select(9, None);
        if phases {
            c.select(3, None);
        }
        c.select(15, None);
        c.tick(true);
        c.tick(true);
        let bytes = c.disk(0).expect("media").tracks()[0].bytes();
        assert_eq!(bytes[0] & 128, if phases { 0 } else { 128 });
        assert_eq!(c.disk(0).expect("media").track_map()[0], 0);
        assert_eq!(c.disk(0).expect("media").tracks()[0].bit_count(), 51200);
    }
}
#[test]
fn unmapped_and_absent_media_rotate_without_read_triggered_advancement() {
    let mut c = card();
    c.insert(
        0,
        Disk::new([255; 160], vec![], 32, true).expect("empty disk"),
    )
    .expect("drive");
    c.select(9, None);
    advance(&mut c, 10000);
    assert!(c.status().bit_positions[0] > 0);
    c.select(11, None);
    advance(&mut c, 10000);
    assert!(c.status().bit_positions[1] > 0);
    assert!(c.eject(0).expect("drive").is_some());
}
