//! Bit tracks and the reference equilibrium-position stepper model.
use crate::DiskError;

/// MSB-first track and its exact wrap point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    bits: Vec<u8>,
    count: u32,
}
impl Track {
    pub fn new(bits: Vec<u8>, bit_count: u32) -> Result<Self, DiskError> {
        if bit_count == 0 || bits.len() != (bit_count as usize).div_ceil(8) {
            return Err(DiskError::Track);
        }
        Ok(Self {
            bits,
            count: bit_count,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bits
    }
    pub fn bit_count(&self) -> u32 {
        self.count
    }
}
/// File-independent media. Write access defaults off; source write protection
/// always wins over a writable work-mount request.
#[derive(Debug, Clone)]
pub struct Disk {
    pub(crate) map: [u8; 160],
    tracks: Vec<Track>,
    timing: u8,
    protected: bool,
    writable: bool,
}
impl Disk {
    pub fn new(
        map: [u8; 160],
        tracks: Vec<Track>,
        bit_timing: u8,
        write_protected: bool,
    ) -> Result<Self, DiskError> {
        if bit_timing == 0 {
            return Err(DiskError::Timing);
        }
        if tracks.len() > 160
            || map
                .iter()
                .any(|&m| m != 255 && usize::from(m) >= tracks.len())
        {
            return Err(DiskError::Map);
        }
        Ok(Self {
            map,
            tracks,
            timing: bit_timing,
            protected: write_protected,
            writable: false,
        })
    }
    pub fn set_writable(&mut self, writable: bool) {
        self.writable = writable;
    }
    pub fn write_protected(&self) -> bool {
        self.protected || !self.writable
    }
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }
    pub fn track_map(&self) -> &[u8; 160] {
        &self.map
    }
}

pub(crate) struct Drive {
    pub disk: Option<Disk>,
    pub head: u8,
    pub position: u32,
    pub reads: u64,
    pub writes: u64,
    phase: u64,
    window: u8,
    noise: u32,
    writing: bool,
    write_ticks: u64,
    write_cursor: u32,
    clock_hz: u32,
}
impl Drive {
    pub fn new(seed: u32) -> Self {
        Self {
            disk: None,
            head: 0,
            position: 0,
            phase: 0,
            window: 0,
            noise: seed,
            writing: false,
            write_ticks: 0,
            write_cursor: 0,
            clock_hz: 8_000_000,
            reads: 0,
            writes: 0,
        }
    }
    fn index(&self) -> Option<usize> {
        let index = self.disk.as_ref()?.map[usize::from(self.head)];
        (index != 255).then_some(usize::from(index))
    }
    fn length(&self) -> u32 {
        self.index().map_or(51200, |i| {
            self.disk.as_ref().expect("index requires disk").tracks[i].count
        })
    }
    pub fn replace(&mut self, disk: Option<Disk>) -> Option<Disk> {
        self.finish_write();
        let old_length = self.length();
        let old = self.disk.take();
        self.disk = disk;
        self.position =
            (u64::from(self.position) * u64::from(self.length()) / u64::from(old_length)) as u32;
        self.window = 0;
        old
    }
    pub fn seek(&mut self, phases: u8) {
        self.finish_write();
        // MAME floppy.cpp seek_phase_w, BSD-3-Clause, Olivier Galibert et al.
        let requested = match phases {
            1 => 0,
            3 => 1,
            2 => 2,
            6 => 3,
            4 => 4,
            12 => 5,
            8 => 6,
            9 => 7,
            _ => return,
        };
        let current = i16::from(self.head);
        if ((current ^ requested) & 7) == 4 {
            return;
        }
        let mut next = (current & !7) | requested;
        if next < current - 4 {
            next += 8;
        } else if next > current + 4 {
            next -= 8;
        }
        let old = self.length();
        self.head = next.clamp(0, 136) as u8;
        self.position =
            (u64::from(self.position) * u64::from(self.length()) / u64::from(old)) as u32;
    }
    pub fn protected(&self, phases: u8) -> bool {
        phases & 2 != 0 || self.disk.as_ref().is_none_or(Disk::write_protected)
    }
    pub fn tick(&mut self, hz: u32, write: bool, phases: u8) -> bool {
        self.clock_hz = hz;
        let enabled = write && !self.protected(phases);
        if self.writing && !enabled {
            self.finish_write();
        }
        if enabled && !self.writing {
            self.writing = true;
            self.write_cursor = self.position;
            self.write_ticks = 0;
        }
        if self.writing {
            self.write_ticks += 1;
        }
        let timing = self.disk.as_ref().map_or(32, |d| d.timing);
        self.phase += 8_000_000;
        let threshold = u64::from(hz) * u64::from(timing);
        if self.phase < threshold {
            return false;
        }
        self.phase -= threshold;
        self.position = (self.position + 1) % self.length();
        let index = self.index();
        let bit = index.map_or(0, |i| {
            let track = &self.disk.as_ref().expect("index requires disk").tracks[i];
            (track.bits[self.position as usize / 8] >> (7 - (self.position & 7))) & 1
        });
        if write {
            return false;
        }
        self.reads = self.reads.wrapping_add(1);
        self.window = (self.window << 1) | bit;
        if self.window & 15 != 0 {
            self.window & 2 != 0
        } else {
            self.noise ^= self.noise << 13;
            self.noise ^= self.noise >> 17;
            self.noise ^= self.noise << 5;
            self.noise % 10 < 3
        }
    }
    pub fn write_transition(&mut self, phases: u8) {
        if !self.protected(phases)
            && let Some(i) = self.index()
        {
            self.clear_write_gap();
            let track = &mut self.disk.as_mut().expect("index requires disk").tracks[i];
            track.bits[self.write_cursor as usize / 8] |= 1 << (7 - (self.write_cursor & 7));
            self.write_ticks = 0;
        }
    }
    // Normalize each interval independently. A physical 4-CPU-cycle write is
    // slightly shorter than the archived 4-us cell. Quantizing absolute angle
    // loses adjacent transitions about once per 50 bits. This WOZ bitstream
    // surface retains decoded cell intervals, not sub-cell flux/splice timing.
    fn clear_write_gap(&mut self) {
        let Some(i) = self.index() else { return };
        let disk = self.disk.as_mut().expect("index requires disk");
        let period = u64::from(self.clock_hz) * u64::from(disk.timing);
        let cells = (self.write_ticks * 8_000_000 + period / 2) / period;
        let track = &mut disk.tracks[i];
        for _ in 0..cells.min(u64::from(track.count)) {
            self.write_cursor = (self.write_cursor + 1) % track.count;
            track.bits[self.write_cursor as usize / 8] &= !(1 << (7 - (self.write_cursor & 7)));
            self.writes = self.writes.wrapping_add(1);
        }
    }
    pub fn finish_write(&mut self) {
        if self.writing {
            self.clear_write_gap();
            self.write_ticks = 0;
            self.writing = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn write_normalization_preserves_adjacent_transitions_at_cpu_write_rate() {
        let mut disk = Disk::new(
            [0; 160],
            vec![Track::new(vec![0; 128], 1024).expect("track")],
            32,
            false,
        )
        .expect("disk");
        disk.set_writable(true);
        let mut drive = Drive::new(1);
        drive.replace(Some(disk));
        for _ in 0..100 {
            for _ in 0..56 {
                drive.tick(14_318_180, true, 0);
            }
            drive.write_transition(0);
        }
        drive.tick(14_318_180, false, 0);
        let recorded = drive.disk.as_ref().expect("disk").tracks[0].bytes();
        let ones: u32 = recorded.iter().map(|v| v.count_ones()).sum();
        assert_eq!(
            ones, 100,
            "absolute-angle quantization loses adjacent pulses"
        );
        for cell in 1..=100 {
            assert_ne!(recorded[cell / 8] & (1 << (7 - cell % 8)), 0);
        }
    }
    #[test]
    fn weak_read_pulses_are_repeatable_noise_and_not_constant_data() {
        let mut a = Drive::new(1);
        let mut b = Drive::new(1);
        let mut high = 0;
        let mut low = 0;
        for _ in 0..10000 {
            let x = a.tick(8_000_000, false, 0);
            let y = b.tick(8_000_000, false, 0);
            assert_eq!(x, y);
            if x {
                high += 1;
            } else {
                low += 1;
            }
        }
        assert!(high > 50 && low > 50);
    }
}
