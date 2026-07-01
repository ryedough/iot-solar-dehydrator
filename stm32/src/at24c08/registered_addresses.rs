use core::array;

use crate::{at24c08::{AT24C08Error, Addresses, ConvRawBytes}, menu::fan_menu::FanSpeed, sht31::SHT31Reading};

pub const SHT_CALIBRATION : Addresses<8, SHT31Reading> = Addresses::new(0,0);
pub const FAN_SPEED : Addresses<1, FanSpeed> = Addresses::new(0,8);

impl ConvRawBytes<8> for SHT31Reading{
    fn from_raw_bytes(bytes : [u8; 8]) -> Result<Self, AT24C08Error> {
        let temp = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let humid = f32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes [7]]);
        if temp.is_nan() | humid.is_nan() {
            Err(AT24C08Error::ConversionError)
        } else {
            Ok(Self{temp, humid})
        }
    }
    fn to_raw_bytes(&self) -> [u8; 8] {
        let mut iter = self.temp.to_be_bytes().into_iter().chain(self.humid.to_be_bytes().into_iter());
        array::from_fn(|_| iter.next().unwrap())
    }
}

impl ConvRawBytes<1> for FanSpeed {
    fn from_raw_bytes(b : [u8; 1]) -> Result<Self, AT24C08Error> {
        match b[0].try_into() {
            Ok(v) => Ok(v),
            Err(_) => Err(AT24C08Error::ConversionError),
        }
    }
    fn to_raw_bytes(&self) -> [u8; 1] {
        [self.clone() as u8]
    }
}
