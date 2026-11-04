//! Apple Disk II card with externally supplied P5/P6 PROMs and bit tracks.
//!
//! Every `tick` is one host crystal edge; `sequencer_edge` is Q3 falling.
//! Bus selects never advance time. No file formats, paths or firmware included.
mod drive;
mod sequencer;
use drive::Drive;
pub use drive::{Disk, Track};
use sequencer::Sequencer;
use std::{error::Error, fmt};

/// Invalid caller-supplied media, drive number or input clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskError {
    Clock,
    Drive,
    Track,
    Map,
    Timing,
}
impl fmt::Display for DiskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Disk II: {self:?}")
    }
}
impl Error for DiskError {}

/// Side-effect-free card and drive observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub data: u8,
    pub sequencer_address: u8,
    pub selected_drive: usize,
    pub motor_on: bool,
    pub phases: u8,
    pub q6: bool,
    pub q7: bool,
    pub quarter_tracks: [u8; 2],
    pub bit_positions: [u32; 2],
    pub bit_reads: [u64; 2],
    pub bit_writes: [u64; 2],
}

/// Card plus two 35-track Disk II mechanisms. Stepping follows the reference's
/// equilibrium-position model; acceleration and head settling are unqualified.
pub struct DiskII {
    boot: [u8; 256],
    sequencer: Sequencer,
    drives: [Drive; 2],
    clock_hz: u32,
    phases: u8,
    selected: usize,
    motor: bool,
    off_remaining: Option<u32>,
    q6: bool,
    q7: bool,
    bus_data: u8,
    pulse_pending: bool,
    write_line: bool,
}
impl DiskII {
    /// PROMs are caller-owned firmware. Host crystal must be >= 8 MHz, so at
    /// most one 125 ns media bit boundary occurs per tick.
    pub fn new(boot: [u8; 256], logic: [u8; 256], crystal_hz: u32) -> Result<Self, DiskError> {
        if crystal_hz < 8_000_000 {
            return Err(DiskError::Clock);
        }
        Ok(Self {
            boot,
            sequencer: Sequencer::new(logic),
            drives: [Drive::new(1), Drive::new(2)],
            clock_hz: crystal_hz,
            phases: 0,
            selected: 0,
            motor: false,
            off_remaining: None,
            q6: false,
            q7: false,
            bus_data: 0,
            pulse_pending: false,
            write_line: false,
        })
    }
    /// Reset card latches, preserving media, head and rotational positions.
    pub fn reset(&mut self) {
        for drive in &mut self.drives {
            drive.finish_write();
        }
        self.phases = 0;
        self.selected = 0;
        self.motor = false;
        self.off_remaining = None;
        self.q6 = false;
        self.q7 = false;
        self.bus_data = 0;
        self.pulse_pending = false;
        self.write_line = false;
        self.sequencer.reset();
    }
    /// Replace media and return the previous image so edits cannot be lost
    /// silently. Archive disks are protected unless explicitly made writable.
    pub fn insert(&mut self, drive: usize, disk: Disk) -> Result<Option<Disk>, DiskError> {
        let drive = self.drives.get_mut(drive).ok_or(DiskError::Drive)?;
        Ok(drive.replace(Some(disk)))
    }
    pub fn eject(&mut self, drive: usize) -> Result<Option<Disk>, DiskError> {
        let drive = self.drives.get_mut(drive).ok_or(DiskError::Drive)?;
        drive.finish_write();
        Ok(drive.replace(None))
    }
    pub fn disk(&self, drive: usize) -> Option<&Disk> {
        self.drives.get(drive)?.disk.as_ref()
    }
    pub fn rom(&self, offset: u8) -> u8 {
        self.boot[usize::from(offset)]
    }
    /// Apply one decoded device select at its bus phase. On writes, supply
    /// the held CPU data bus before intervening sequencer clocks occur.
    pub fn select(&mut self, offset: u8, write: Option<u8>) {
        if let Some(data) = write {
            self.bus_data = data;
        }
        let offset = offset & 15;
        match offset {
            0..=7 => {
                let mask = 1 << (offset / 2);
                if offset & 1 != 0 {
                    self.phases |= mask;
                } else {
                    self.phases &= !mask;
                }
                if self.motor {
                    self.drives[self.selected].seek(self.phases);
                }
            }
            8 => {
                if self.motor && self.off_remaining.is_none() {
                    self.off_remaining = Some(self.clock_hz);
                }
            }
            9 => {
                if !self.motor {
                    self.sequencer.reset();
                    self.pulse_pending = false;
                    self.write_line = false;
                }
                self.motor = true;
                self.off_remaining = None;
                self.drives[self.selected].seek(self.phases);
            }
            10 | 11 => {
                self.drives[self.selected].finish_write();
                self.selected = usize::from(offset & 1);
                if self.motor {
                    self.drives[self.selected].seek(self.phases);
                }
            }
            12 | 13 => self.q6 = offset & 1 != 0,
            _ => self.q7 = offset & 1 != 0, // Masked offset is 14 or 15.
        }
    }
    /// Data is driven only for even device-select addresses. Odd addresses
    /// leave the host floating bus visible. This observation has no effects.
    pub fn data(&self, offset: u8) -> Option<u8> {
        if offset & 1 == 0 {
            Some(self.sequencer.data)
        } else {
            None
        }
    }
    /// Advance one host crystal tick, using the host's actual Q3 edge.
    pub fn tick(&mut self, sequencer_edge: bool) {
        if let Some(left) = self.off_remaining.as_mut() {
            *left -= 1;
            if *left == 0 {
                self.drives[self.selected].finish_write();
                self.motor = false;
                self.off_remaining = None;
            }
        }
        if !self.motor {
            return;
        }
        let drive = &mut self.drives[self.selected];
        self.pulse_pending |= drive.tick(self.clock_hz, self.q7, self.phases);
        if sequencer_edge {
            let write = self.sequencer.address & 0x80 != 0;
            if self.q7 && write != self.write_line {
                drive.write_transition(self.phases);
            }
            self.write_line = write;
            self.sequencer.tick(
                self.q6,
                self.q7,
                self.pulse_pending,
                drive.protected(self.phases),
                self.bus_data,
            );
            self.pulse_pending = false;
        }
    }
    pub fn status(&self) -> Status {
        Status {
            data: self.sequencer.data,
            sequencer_address: self.sequencer.address,
            selected_drive: self.selected,
            motor_on: self.motor,
            phases: self.phases,
            q6: self.q6,
            q7: self.q7,
            quarter_tracks: self.drives.each_ref().map(|d| d.head),
            bit_positions: self.drives.each_ref().map(|d| d.position),
            bit_reads: self.drives.each_ref().map(|d| d.reads),
            bit_writes: self.drives.each_ref().map(|d| d.writes),
        }
    }
}
