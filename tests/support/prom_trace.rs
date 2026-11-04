use super::Sequencer;
#[test]
#[ignore = "external P6 PROM; set EMU198X_DISK_II_P6"]
fn original_p6_matches_the_independently_executed_mame_trace() {
    let bytes =
        std::fs::read(std::env::var("EMU198X_DISK_II_P6").expect("set P6")).expect("read P6");
    let mut s = Sequencer::new(bytes.try_into().expect("256 P6 bytes"));
    for (line, row) in include_str!("../fixtures/mame-p6.csv").lines().enumerate() {
        let v: Vec<u8> = row
            .split(',')
            .map(|v| v.parse().expect("trace number"))
            .collect();
        assert_eq!(v.len(), 7);
        s.tick(v[0] != 0, v[1] != 0, v[2] != 0, v[3] != 0, v[4]);
        assert_eq!((s.address, s.data), (v[5], v[6]), "trace row {line}");
    }
}
