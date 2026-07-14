use core::array;

use heapless::{String, Vec};

use crate::{at24c08::{AT24C08Error, Addresses, ConvRawBytes}, menu::fan_menu::FanSpeed, sht31::SHT31Reading};

// ----------------- IMPORTANT ------------------
// make sure the address page * 16 + offset + len
// never exceed last address on eeprom
// which is 1024

pub const SHT_CALIBRATION : Addresses<8, SHT31Reading> = Addresses::new(0,0);
pub const FAN_SPEED : Addresses<1, FanSpeed> = Addresses::new(0,8);
pub const CONNECTED_WIFI : Addresses<96, WifiConfig> = Addresses::new(1, 0);

pub struct WifiConfig {
    pub has_connected : bool,
    pub ssid : String<32>,
    pub password : String<63>,
}

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

impl ConvRawBytes<96> for WifiConfig {
    fn to_raw_bytes(&self) -> [u8; 96] {
        let mut iter = [self.has_connected as u8].into_iter()
            .chain(self.ssid.as_bytes().into_iter().copied())
            .chain(self.password.as_bytes().into_iter().copied());
        array::from_fn(|_| iter.next().unwrap())
    }
    fn from_raw_bytes(b : [u8; 96]) -> Result<Self, AT24C08Error> {
        let has_connected = b[0] == 1;
        let ssid : Vec<u8, 32> = match Vec::from_slice(&b[1..33]) {
            Ok(v) => v,
            Err(_) => return Err(AT24C08Error::ConversionError),
        };
        let ssid = String::from_utf8(ssid).unwrap();
        let password : Vec<u8, 63> = match Vec::from_slice(&b[33..]) {
            Ok(v) => v,
            Err(_) => return Err(AT24C08Error::ConversionError),
        };
        let password = match String::from_utf8(password){
            Ok(v) => v,
            Err(_) => return Err(AT24C08Error::ConversionError),
        };

        Ok(Self{ssid, password, has_connected})
    }
}
