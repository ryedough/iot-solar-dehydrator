use core::array::{self, IntoIter};

use embassy_futures::select::Either;
use embassy_time::Duration;
use heapless::{String, Vec};

use crate::{at24c08::{AT24C08, AT24C08Error, Addresses, ConvRawBytes, configs::WifiConfig}, esp12f::ConnWifi, menu::fan_menu::FanSpeed, sht31::SHT31Reading};

// ----------------- IMPORTANT ------------------
// make sure the address page * 16 + offset + len
// never overwrite other address
// AND
// never exceed last address on eeprom
// which is 1024

pub const WIFI_CONFIG : Addresses<104, WifiConfig> = Addresses::new(0, 0);
pub const SHT_CALIBRATION : Addresses<8, SHT31Reading> = Addresses::new(0,1);
pub const FAN_SPEED : Addresses<1, FanSpeed> = Addresses::new(0,9);
pub const HAS_INITIATED : Addresses<1, bool> = Addresses::new(0,9); // track if at24c08 has been

pub struct Settings {
    pub calibration : SHT31Reading,
    pub fan_speed : FanSpeed,
    pub wifi_config : WifiConfig,
}

impl Settings {
    pub async fn load(eeprom : &AT24C08)-> Settings{
        if !eeprom.read(&HAS_INITIATED).await.unwrap() {
            let calibration = SHT31Reading::default();
            let fan_speed = FanSpeed::Medium;
            let wifi_config = WifiConfig::default();

            eeprom.write(&HAS_INITIATED, &true).await.unwrap();
            eeprom.write(&SHT_CALIBRATION, &calibration).await.unwrap();
            eeprom.write(&FAN_SPEED, &fan_speed).await.unwrap();
            eeprom.write(&WIFI_CONFIG, &wifi_config).await.unwrap();

            return Settings {
                calibration,
                fan_speed,
                wifi_config,
            };
        }
        let calibration = eeprom.read_or_default(&SHT_CALIBRATION).await;
        let fan_speed = eeprom.read_or_default(&FAN_SPEED).await;
        let wifi_config = eeprom.read_or_default(&WIFI_CONFIG).await;

        Settings {
            calibration,
            fan_speed,
            wifi_config,
        }
    }
}

impl ConvRawBytes<8> for SHT31Reading{
    fn from_raw_bytes(bytes : [u8; 8]) -> Result<Self, AT24C08Error> {
        let temp = f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let humid = f32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes [7]]);
        if temp.is_nan() | humid.is_nan() {
            Err(AT24C08Error::DataCorrupted)
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
            Err(_) => Err(AT24C08Error::DataCorrupted),
        }
    }
    fn to_raw_bytes(&self) -> [u8; 1] {
        [self.clone() as u8]
    }
}

impl ConvRawBytes<104> for WifiConfig {
    fn to_raw_bytes(&self) -> [u8; 104] {
        let mut buf = [0u8; 104];
        match &self.wifi {
            Some(wifi) => {
                buf[0] = true as u8;
                (&mut buf[1..wifi.ssid.len()]).copy_from_slice(wifi.ssid.as_bytes());
                (&mut buf[33..wifi.password.len()]).copy_from_slice(wifi.password.as_bytes());
            }
            None => {
                buf[0] = false as u8;
            }
        }
        (&mut buf[96..]).copy_from_slice(&self.interval.as_millis().to_be_bytes());
        buf
    }
    fn from_raw_bytes(b : [u8; 104]) -> Result<Self, AT24C08Error> {
        let has_conn = b[0] == true as u8;
        let wifi = if has_conn {
            let ssid : Vec<u8, 32> = match Vec::from_slice(&b[1..33]) {
                Ok(v) => v,
                Err(_) => return Err(AT24C08Error::DataCorrupted),
            };
            let ssid = String::from_utf8(ssid).unwrap();
            let password : Vec<u8, 63> = match Vec::from_slice(&b[33..96]) {
                Ok(v) => v,
                Err(_) => return Err(AT24C08Error::DataCorrupted),
            };
            let password = match String::from_utf8(password){
                Ok(v) => v,
                Err(_) => return Err(AT24C08Error::DataCorrupted),
            };
            Some(ConnWifi{ssid, password})
        }else {
            None
        };
        let interval : [u8; _] = array::from_fn(|i| b[i + 96]);
        let interval = u64::from_be_bytes(interval);
        let interval = Duration::from_millis(interval);

        Ok(Self{interval, wifi})
    }
}

impl ConvRawBytes<1> for bool {
    fn from_raw_bytes(b : [u8; 1]) -> Result<Self, AT24C08Error> {
        Ok(b[0] == 1)
    }
    fn to_raw_bytes(&self) -> [u8; 1] {
        [self.clone() as u8]
    }
}
