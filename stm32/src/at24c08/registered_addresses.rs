use core::array;

use crate::{at24c08::{Addresses, ConvRawBytes}, sht31::SHT31Reading};

pub const SHT_CALIBRATION : Addresses<8, SHT31Reading> = Addresses::new(0,0);

impl ConvRawBytes<8> for SHT31Reading{
    fn from_raw_bytes(bytes : [u8; 8]) -> Self {
        Self {
            temp : f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            humid : f32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes [7]]),
        }
    }
    fn to_raw_bytes(&self) -> [u8; 8] {
        let mut iter = self.temp.to_be_bytes().into_iter().chain(self.humid.to_be_bytes().into_iter());
        array::from_fn(|_| iter.next().unwrap())
    }
}
