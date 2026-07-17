use embassy_time::Duration;

use crate::esp12f::ConnWifi;

#[derive(Clone)]
pub struct WifiConfig {
    pub wifi : Option<ConnWifi>,
    pub interval : Duration
}

impl Default for WifiConfig {
    fn default() -> Self {
        Self {
            wifi : core::default::Default::default(),
            interval : Duration::from_millis(15000),
        }
    }
}
