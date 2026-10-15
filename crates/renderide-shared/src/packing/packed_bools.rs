//! Eight boolean flags packed into one byte for a compact wire representation.

/// Eight flags read from one byte: `bit0` is the least significant bit (same ordering as the wire format).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedBools {
    /// Wire bit 0 (LSB of the packed byte).
    pub bit0: bool,
    /// Wire bit 1.
    pub bit1: bool,
    /// Wire bit 2.
    pub bit2: bool,
    /// Wire bit 3.
    pub bit3: bool,
    /// Wire bit 4.
    pub bit4: bool,
    /// Wire bit 5.
    pub bit5: bool,
    /// Wire bit 6.
    pub bit6: bool,
    /// Wire bit 7 (MSB of the packed byte).
    pub bit7: bool,
}

impl PackedBools {
    /// Decodes a byte from the wire into eight flags.
    #[inline]
    pub const fn from_byte(byte: u8) -> Self {
        Self {
            bit0: (byte & 1) != 0,
            bit1: (byte & 2) != 0,
            bit2: (byte & 4) != 0,
            bit3: (byte & 8) != 0,
            bit4: (byte & 0x10) != 0,
            bit5: (byte & 0x20) != 0,
            bit6: (byte & 0x40) != 0,
            bit7: (byte & 0x80) != 0,
        }
    }

    /// First two flags.
    #[inline]
    pub const fn two(self) -> (bool, bool) {
        (self.bit0, self.bit1)
    }

    /// First three flags.
    #[inline]
    pub const fn three(self) -> (bool, bool, bool) {
        (self.bit0, self.bit1, self.bit2)
    }

    /// First four flags.
    #[inline]
    pub const fn four(self) -> (bool, bool, bool, bool) {
        (self.bit0, self.bit1, self.bit2, self.bit3)
    }

    /// First five flags.
    #[inline]
    pub const fn five(self) -> (bool, bool, bool, bool, bool) {
        (self.bit0, self.bit1, self.bit2, self.bit3, self.bit4)
    }

    /// First six flags.
    #[inline]
    pub const fn six(self) -> (bool, bool, bool, bool, bool, bool) {
        (
            self.bit0, self.bit1, self.bit2, self.bit3, self.bit4, self.bit5,
        )
    }

    /// First seven flags.
    #[inline]
    pub const fn seven(self) -> (bool, bool, bool, bool, bool, bool, bool) {
        (
            self.bit0, self.bit1, self.bit2, self.bit3, self.bit4, self.bit5, self.bit6,
        )
    }

    /// All eight flags.
    #[inline]
    pub const fn eight(self) -> (bool, bool, bool, bool, bool, bool, bool, bool) {
        (
            self.bit0, self.bit1, self.bit2, self.bit3, self.bit4, self.bit5, self.bit6, self.bit7,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_byte_zero_yields_all_false() {
        let pb = PackedBools::from_byte(0x00);
        assert_eq!(
            pb,
            PackedBools {
                bit0: false,
                bit1: false,
                bit2: false,
                bit3: false,
                bit4: false,
                bit5: false,
                bit6: false,
                bit7: false,
            }
        );
    }

    #[test]
    fn from_byte_ff_yields_all_true() {
        let pb = PackedBools::from_byte(0xFF);
        assert_eq!(pb.eight(), (true, true, true, true, true, true, true, true));
    }

    #[test]
    fn from_byte_walks_each_bit_independently() {
        for i in 0..8u32 {
            let byte = 1u8 << i;
            let pb = PackedBools::from_byte(byte);
            let bits = [
                pb.bit0, pb.bit1, pb.bit2, pb.bit3, pb.bit4, pb.bit5, pb.bit6, pb.bit7,
            ];
            for (j, set) in bits.iter().enumerate() {
                assert_eq!(
                    *set,
                    j as u32 == i,
                    "byte=0x{byte:02X} bit{j} expected {} got {set}",
                    j as u32 == i,
                );
            }
        }
    }

    #[test]
    fn tuple_accessors_match_underlying_fields() {
        let pb = PackedBools::from_byte(0xA5);
        let bits = (true, false, true, false, false, true, false, true);
        assert_eq!(pb.two(), (bits.0, bits.1));
        assert_eq!(pb.three(), (bits.0, bits.1, bits.2));
        assert_eq!(pb.four(), (bits.0, bits.1, bits.2, bits.3));
        assert_eq!(pb.five(), (bits.0, bits.1, bits.2, bits.3, bits.4));
        assert_eq!(pb.six(), (bits.0, bits.1, bits.2, bits.3, bits.4, bits.5));
        assert_eq!(
            pb.seven(),
            (bits.0, bits.1, bits.2, bits.3, bits.4, bits.5, bits.6)
        );
        assert_eq!(pb.eight(), bits);
    }

    #[test]
    fn default_yields_all_false() {
        let pb = PackedBools::default();
        assert_eq!(
            pb.eight(),
            (false, false, false, false, false, false, false, false),
        );
    }
}
