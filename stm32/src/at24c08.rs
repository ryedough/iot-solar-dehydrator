use crate::I2C;
use core::marker::PhantomData;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_async::i2c::Operation;

pub mod registered_addresses;

pub trait ConvRawBytes<const LEN: usize> {
    fn from_raw_bytes(b : [u8; LEN]) -> Self;
    fn to_raw_bytes(&self) -> [u8; LEN];
}

enum RWBit {
    Read = 1,
    Write = 0,
}

const AT24C08_ADDRESS: u8 = 0b1010000;
const T_WRITE_CYCLE: Duration = Duration::from_millis(5);

pub struct Addresses<const LEN: usize, T: ConvRawBytes<LEN>>{
    page: u8,
    offset: u8,
    phantom: PhantomData<T>,
}
impl<const LEN: usize, T: ConvRawBytes<LEN>> Addresses<LEN, T> {
    const fn new(page: u8, offset: u8) -> Self {
        assert!(page < 64);
        assert!(offset < 16);
        assert!(LEN > 0);
        assert!(offset as usize + LEN - 1 < 16);

        Self {
            page,
            offset,
            phantom: PhantomData,
        }
    }
    fn as_address(&self) -> u16 {
        self.page as u16 * 16 + self.offset as u16
    }
}

pub struct AT24C08 {
    last_write_cycle: Option<Instant>,
}

impl AT24C08 {
    pub fn new()->Self{
        Self{
            last_write_cycle : None
        }
    }
    fn get_address<const LEN: usize, T: ConvRawBytes<LEN>>(
        &self,
        address: Addresses<LEN, T>,
        rw: RWBit,
    ) -> [u8; 2] {
        let u16address = address.as_address();
        let device_address = AT24C08_ADDRESS | ((u16address >> 8) as u8) << 1 | rw as u8;
        let word_address = u16address as u8;
        [device_address, word_address]
    }
    async fn wait_write_cycle(&mut self) {
        match self.last_write_cycle {
            Some(lwc) => {
                let elapsed = lwc.elapsed();
                if elapsed > T_WRITE_CYCLE {
                    let _ = self.last_write_cycle.take();
                } else {
                    Timer::after_millis(T_WRITE_CYCLE.as_millis() - elapsed.as_millis()).await;
                    let _ = self.last_write_cycle.take();
                }
            }
            None => {}
        }
    }
    pub async fn read<const LEN: usize, T: ConvRawBytes<LEN>>(
        &mut self,
        address: Addresses<LEN, T>,
    ) -> Result<T, embassy_stm32::i2c::Error> {
        self.wait_write_cycle().await;
        let address = self.get_address(address, RWBit::Read);
        let mut reading = [0; LEN];
        I2C.lock()
            .await
            .as_mut()
            .unwrap()
            .transaction(
                address[0] & !(RWBit::Read as u8),
                &mut [Operation::Write(&[address[1]])],
            )
            .await?;
        I2C.lock()
            .await
            .as_mut()
            .unwrap()
            .transaction(address[0], &mut [Operation::Read(&mut reading)])
            .await?;
        Ok(T::from_raw_bytes(reading))
    }
    pub async fn write<const LEN: usize, T: ConvRawBytes<LEN>>(
        &mut self,
        address: Addresses<LEN, T>,
        value: T,
    ) -> Result<(), embassy_stm32::i2c::Error> {
        self.wait_write_cycle().await;
        let address = self.get_address(address, RWBit::Write);
        let r = I2C
            .lock()
            .await
            .as_mut()
            .unwrap()
            .transaction(
                address[0],
                &mut [Operation::Write(&[address[1]]), Operation::Write(&value.to_raw_bytes())],
            )
            .await;
        let _ = self.last_write_cycle.insert(Instant::now());
        r
    }
}
