use embassy_executor::task;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, watch::Sender};
use embassy_time::Timer;
use embedded_hal_async::i2c::Operation;

use crate::{I2C, channeled_signal::{SignalReceiver, SignalSender}};

#[derive(Debug)]
pub enum SHT31Error {
    NotConnected,
    NoResponse,
}

impl defmt::Format for SHT31Error {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            SHT31Error::NotConnected => defmt::write!(fmt, "SHT31 is not connected"),
            SHT31Error::NoResponse => defmt::write!(fmt, "SHT31 wont respond to read request"),
        }
    }
}

const SHT31_ADDRESS : u8 = 0x44;
pub struct SHT31 {
    calibration : SHT31Reading,
}

#[derive(Clone, Copy, Default)]
pub struct SHT31Reading {
    pub temp : f32,
    pub humid : f32,
}

impl SHT31 {
    pub fn new(calibration : SHT31Reading) -> Self {
        return Self {calibration}
    }
    pub async fn get_climate(&self) -> Result<SHT31Reading, SHT31Error> {
        if let Err(_) = I2C.lock().await.as_mut().unwrap().transaction(SHT31_ADDRESS, &mut [
            Operation::Write(&[0x24, 0x00]), // high repeatability
        ]).await {
            return Err(SHT31Error::NotConnected);
        };

        Timer::after_millis(15).await; //delay wait reading

        let mut reading = [0; 6];
        if let Err(_) = I2C.lock().await.as_mut().unwrap().read(SHT31_ADDRESS, &mut reading).await {
            return Err(SHT31Error::NoResponse);
        };
        let temp = u16::from_be_bytes([reading[0], reading[1]]);
        let temp = -45. + (175. * (temp as f32 / 65535.)) + self.calibration.temp;
        let humid = u16::from_be_bytes([reading[3], reading[4]]);
        let humid = 100. * (humid as f32 / 65535.) + self.calibration.humid;

        return Ok(SHT31Reading{
            temp, humid,
        })
    }
}

#[task]
pub async fn read_sht_task(
    calibration_sr: SignalReceiver<SHT31Reading>,
    climate_ws: Sender<'static, ThreadModeRawMutex, SHT31Reading, 2>,
) {
    let calibration = calibration_sr.receive().await;
    let mut sht31 = SHT31::new(calibration);

    loop {
        if let Some(calibration) = calibration_sr.try_receive() {
            sht31 = SHT31::new(calibration);
        }
        match sht31.get_climate().await {
            Ok(sht31_reading) => climate_ws.send(sht31_reading),
            Err(err) => defmt::error!("{:?}", err),
        }
        Timer::after_secs(5).await;
    }
}
