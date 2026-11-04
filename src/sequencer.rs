//! P6 PROM truth table, following MAME wozfdc.cpp (Olivier Galibert,
//! BSD-3-Clause). See THIRD_PARTY_NOTICES.md. No firmware is embedded.
pub(crate) struct Sequencer {
    prom: [u8; 256],
    pub data: u8,
    pub address: u8,
}
impl Sequencer {
    pub fn new(prom: [u8; 256]) -> Self {
        Self {
            prom,
            data: 0,
            address: 0,
        }
    }
    pub fn reset(&mut self) {
        self.data = 0;
        self.address = 0;
    }
    pub fn tick(&mut self, q6: bool, q7: bool, pulse: bool, protected: bool, bus: u8) {
        self.address = (self.address & 0xe1)
            | (u8::from(self.data & 128 != 0) << 1)
            | (u8::from(q6) << 2)
            | (u8::from(q7) << 3)
            | (u8::from(!pulse) << 4);
        let opcode = self.prom[usize::from(self.address)];
        self.address = (self.address & 0x1e)
            | (opcode & 0xc0)
            | ((opcode & 0x20) >> 5)
            | ((opcode & 0x10) << 1);
        self.data = match opcode & 15 {
            0..=7 => 0,
            8 | 12 => self.data,
            9 => self.data << 1,
            10 | 14 => (self.data >> 1) | (u8::from(protected) << 7),
            11 | 15 => bus,
            13 => (self.data << 1) | 1,
            _ => unreachable!(),
        };
        self.address = (self.address & !2) | (u8::from(self.data & 128 != 0) << 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_data_operation_and_feedback_bit_follows_the_logic_outputs() {
        for opcode in 0..16u8 {
            let mut s = Sequencer::new([opcode; 256]);
            s.data = 0x81;
            s.tick(true, false, false, true, 0x42);
            let expected = match opcode {
                0..=7 => 0,
                8 | 12 => 0x81,
                9 => 2,
                10 | 14 => 0xc0,
                11 | 15 => 0x42,
                13 => 3,
                _ => unreachable!(),
            };
            assert_eq!(s.data, expected);
            assert_eq!(s.address & 2, u8::from(expected & 128 != 0) * 2);
        }
    }
    #[test]
    fn partition_inputs_and_permuted_next_state_select_the_prom() {
        let mut prom = [0; 256];
        prom[0x1e] = 0xf8;
        let mut s = Sequencer::new(prom);
        s.data = 0x80;
        s.tick(true, true, false, false, 0);
        assert_eq!(s.data, 0x80);
        assert_eq!(s.address, 0xff);
        s.tick(false, false, true, false, 0);
        assert_eq!(s.data, 0);
    }
}

#[cfg(test)]
#[path = "../tests/support/prom_trace.rs"]
mod prom_trace;
