use crate::at24c08::{Addresses, FromRawBytes};

pub const TEST : Addresses<1, u8> = Addresses::new(0, 0);

impl FromRawBytes<1> for u8 {
    fn from_raw_bytes(bytes : [u8; 1]) -> Self {
        bytes[0]
    }
}
