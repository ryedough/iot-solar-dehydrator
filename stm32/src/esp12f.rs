use core::array;

use defmt::info;
use embassy_stm32::{exti::ExtiInput, gpio::Output, mode::Async, spi::{Spi, mode::Master}};
use embassy_time::{Duration, TimeoutError, Timer, WithTimeout};
use heapless::{CString, String, Vec};

const READ_BUFFER_LEN : usize = 2048;

pub struct ESP12F {
    spi : Spi<'static, Async, Master>,
    spi_handshake : ExtiInput<'static, Async>,
    spi_cs : Output<'static>,
    reset_pin : Output<'static>,
}

enum Commands {
    Scan = 0x0,
    Connect = 0x1,
    SendClimate = 0x2,
    Sleep = 0x3
}

#[derive(Debug, defmt::Format)]
pub struct WifiScan {
    ssid : String<33>,
    rssi : u8,
}

pub struct ESP12FActive<'a> {
    spi : &'a mut Spi<'static, Async, Master>,
    spi_handshake : &'a mut ExtiInput<'static, Async>,
    spi_cs : &'a mut Output<'static>,
}

impl<'a> ESP12FActive<'a> {
    async fn write_status(&mut self, length : u32) -> Result<(), TimeoutError> {
        let write_bit: [u8;_] = [0x1];
        let mut data = write_bit.into_iter().chain(length.to_le_bytes().into_iter());
        let data: [u8; 5] = array::from_fn(|_| data.next().unwrap());
        self.spi_cs.set_low();
        self.spi.write(&data).await.unwrap();
        self.spi_cs.set_high();
        self.spi_handshake.wait_for_high().with_timeout(Duration::from_millis(100)).await
    }
    async fn read_status(&mut self) -> Result<u32,TimeoutError> {
        self.spi_handshake.wait_for_high()
            .with_timeout(Duration::from_millis(10000)).await?;
        let read_bit: [u8;_] = [0x4, 0x0, 0x0, 0x0, 0x0];
        let mut read_data = [0; 5];

        self.spi_cs.set_low();
        self.spi.transfer(&mut read_data, &read_bit).await.unwrap();
        self.spi_cs.set_high();
        Ok(u32::from_le_bytes([read_data[1], read_data[2], read_data[3], read_data[4]]))
    }
    async fn read(&mut self) -> Result<Vec<u8, READ_BUFFER_LEN>, TimeoutError> {
        let data_len = loop {
            let data_len = self.read_status().await.unwrap();
            match data_len {
                0xffff_ffff | 0x0 => Timer::after_millis(20).await,
                _ => break data_len as usize,
            } // hacky way to fix if read too early after esp boot, it will return 0 or max u32
              // TODO: make esp handshake ceremony at esp startup instead
        };

        defmt::info!("data len: {}", data_len);
        let mut parsed : Vec<u8, READ_BUFFER_LEN> = Vec::new();

        for chunk_len in (0..data_len).step_by(64).map(|i| core::cmp::min(data_len-i, 64)){
            Timer::after_millis(50).await;
            self.spi_handshake.wait_for_high().await;

            // 2-byte command header + space to receive payload
            let mut rx = [0u8; 66];
            let mut tx = [0u8; 66];

            tx[0] = 0x03; // Read Data
            tx[1] = 0x00; // Address

            self.spi_cs.set_low();
            self.spi.transfer(&mut rx[..chunk_len + 2], &tx[..chunk_len + 2])
                .await
                .unwrap();
            self.spi_cs.set_high();

            parsed.extend_from_slice(&rx[2..chunk_len + 2]).unwrap();
        }
        Ok(parsed)
    }
    async fn write(&mut self, data: &[u8], force: bool) -> Result<(), TimeoutError> {
        while let Err(_) =  self.write_status(data.len() as u32).await {};
            // hacky way to fix if write too early, esp wont pull up the handshake pin
            // TODO: make esp handshake ceremony at esp startup instead
        let mut packet = [0u8; 66];
        packet[0] = 0x02;
        packet[1] = 0x00;
        for d in data.chunks(64) {
            self.spi_handshake.wait_for_high().await;

            packet[2..2 + d.len()].copy_from_slice(d);

            self.spi_cs.set_low();
            self.spi.write(&packet[..2 + d.len()]).await.unwrap();
            self.spi_cs.set_high();
        }
        self.spi_handshake.wait_for_high().await;
        if force {
            while let Err(_) =  self.write_status(data.len() as u32).await {};
        } else {
            self.write_status(0).await;
        };
        Ok(())
    }
    pub async fn scan(&mut self)-> Result<Vec<WifiScan, 10>,TimeoutError>{
        self.write(&[Commands::Scan as u8], true).await?;
        let parsed  = self.read().await?;
        let mut result: Vec<WifiScan, 10> = Vec::new();
        for chunk in parsed.chunks(34) {
            let ssid: CString<33> = CString::from_bytes_truncating_at_nul(chunk).unwrap();
            let ssid: String<33> = ssid.into_string().unwrap();
            let rssi = chunk[33];
            result.push(WifiScan { ssid, rssi }).unwrap();
        };
        Ok(result)
    }
    pub async fn sleep(mut self) {
        self.write(&[Commands::Sleep as u8], false).await;
        Timer::after_millis(100).await;
    }
}

impl<'a> ESP12F {
    pub fn new(spi: Spi<'static, Async, Master>, spi_handshake :ExtiInput<'static, Async>, spi_cs: Output<'static>, reset_pin : Output<'static> )->Self{
        Self{
            spi,
            reset_pin,
            spi_handshake,
            spi_cs
        }
    }
    pub async fn turn_on(&'a mut self)->ESP12FActive<'a>{
        self.spi_cs.set_low();
        self.reset_pin.set_low();
        Timer::after_millis(50).await;
        self.reset_pin.set_high();
        Timer::after_millis(50).await;
        self.spi_cs.set_high();
        Timer::after_millis(500).await;
        ESP12FActive{
            spi_cs : &mut self.spi_cs,
            spi_handshake : &mut self.spi_handshake,
            spi : &mut self.spi,
        }
    }
}
