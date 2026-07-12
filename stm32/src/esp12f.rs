use core::array;

use embassy_stm32::{exti::ExtiInput, gpio::Output, mode::Async, spi::{Spi, mode::Master}};
use embassy_time::{Duration, TimeoutError, Timer, WithTimeout};
use heapless::Vec;

pub struct ESP12F {
    spi : Spi<'static, Async, Master>,
    spi_handshake : ExtiInput<'static, Async>,
    spi_cs : Output<'static>,
    reset_pin : Output<'static>,
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
            .with_timeout(Duration::from_millis(1000)).await?;
        let read_bit: [u8;_] = [0x4];
        self.spi_cs.set_low();
        self.spi.write(&read_bit).await.unwrap();
        self.spi_cs.set_high();
        let mut read_data = [0u8; 4];
        self.spi.read(&mut read_data).await.unwrap();
        Ok(u32::from_le_bytes(read_data))
    }
    pub async fn read(&mut self) {
        let data_len = self.read_status().await.unwrap();
        defmt::info!("{}",data_len);
    }
    pub async fn write(&mut self, data: &[u8]) {
        while let Err(_) =  self.write_status(data.len() as u32).await {};
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
        self.write_status(0).await.unwrap();
    }
}
impl<'a> Drop for ESP12FActive<'a> {
    fn drop(&mut self) {
        self.spi_cs.set_low();
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
        self.reset_pin.set_high();
        Timer::after_millis(50).await;
        self.spi_cs.set_high();
        ESP12FActive{
            spi_cs : &mut self.spi_cs,
            spi_handshake : &mut self.spi_handshake,
            spi : &mut self.spi,
        }
    }
}
