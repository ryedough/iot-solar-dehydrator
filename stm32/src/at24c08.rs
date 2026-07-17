use crate::{I2C, at24c08::registered_addresses::Settings};
use core::marker::PhantomData;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_async::i2c::Operation;

pub mod registered_addresses;
pub mod configs;

pub trait ConvRawBytes<const LEN: usize> where Self: Sized {
    fn from_raw_bytes(b : [u8; LEN]) -> Result<Self, AT24C08Error>;
    fn to_raw_bytes(&self) -> [u8; LEN];
}

enum RWBit {
    Read = 1,
    Write = 0,
}

#[derive(Debug, Clone, Copy)]
pub enum AT24C08Error {
    I2CError,
    DataCorrupted,
}

impl From<embassy_stm32::i2c::Error> for AT24C08Error {
    fn from(value: embassy_stm32::i2c::Error) -> Self {
        Self::I2CError
    }
}

const AT24C08_ADDRESS: u8 = 0b1010000;
const T_WRITE_CYCLE: Duration = Duration::from_millis(5);

pub struct Addresses<const LEN: usize, T: ConvRawBytes<LEN>>{
    page: u8,
    offset: u8,
    absolute : u16, //actually only 10 bit, (0..1023)
    word : u8, //absolute address but only the lowest 8 bits
    phantom: PhantomData<T>,
}
impl<const LEN: usize, T: ConvRawBytes<LEN>> Addresses<LEN, T> {
    const fn new(page: u8, offset: u8) -> Self {
        assert!(page < 64);
        assert!(offset < 16);
        assert!(LEN > 0);
        let absolute= page as u16 * 16 + offset as u16;

        Self {
            page,
            offset,
            absolute,
            word : absolute as u8,
            phantom: PhantomData,
        }
    }

    fn device(&self, rw: RWBit) -> u8 {
        AT24C08_ADDRESS | ((self.absolute >> 8) as u8) << 1 | rw as u8
    }
}


pub struct AT24C08 {}

impl AT24C08 {

    pub async fn new_and_load_settings()->(Self, Settings) {
        let eeprom = Self{};
        let settings = Settings::load(&eeprom).await;
        (eeprom, settings)
    }

    /// Will also write value into eeprom if this function caught DataCorrupted Error
    pub async fn read_or_default<T : ConvRawBytes<LEN> + Default, const LEN : usize>(
        &self,
        addr: &Addresses<LEN, T>,
    ) -> T{
        let default = T::default();
        loop {
            match self.read(addr).await {
                Ok(v) => return v,
                Err(AT24C08Error::DataCorrupted) => {
                    self.write(addr, &default).await.unwrap();
                    return default;
                }
                Err(AT24C08Error::I2CError) => {
                    defmt::error!("EEPROM not connected, retrying in 5 seconds");
                    Timer::after_secs(5).await;
                }
            }
        }
    }
    pub async fn read<const LEN: usize, T: ConvRawBytes<LEN>>(
        &self,
        address: &Addresses<LEN, T>,
    ) -> Result<T, AT24C08Error> {
        let mut reading = [0; LEN];
        I2C.lock()
            .await
            .as_mut()
            .unwrap()
            .transaction(
                address.device(RWBit::Read) & !(RWBit::Read as u8),
                &mut [Operation::Write(&[address.word])],
            )
            .await?;
        I2C.lock()
            .await
            .as_mut()
            .unwrap()
            .transaction(address.device(RWBit::Read), &mut [Operation::Read(&mut reading)])
            .await?;
        T::from_raw_bytes(reading)
    }
    pub async fn write<const LEN: usize, T: ConvRawBytes<LEN>>(
        &self,
        address: &Addresses<LEN, T>,
        value: &T,
    ) -> Result<(), embassy_stm32::i2c::Error> {
        let data = value.to_raw_bytes();
        let mut remaining = &data[..];

        let mut write_address = address.absolute;
        while !remaining.is_empty() {
            let page_offset = (write_address & 0x0F) as usize;
            let chunk_len = core::cmp::min(16 - page_offset, remaining.len());

            I2C.lock()
                .await
                .as_mut()
                .unwrap()
                .transaction(
                    address.device(RWBit::Write),
                    &mut [
                        Operation::Write(&[address.word]),
                        Operation::Write(&remaining[..chunk_len]),
                    ],
                )
                .await?;

            write_address += chunk_len as u16;
            remaining = &remaining[chunk_len..];

            Timer::after(T_WRITE_CYCLE).await;
        }

        Ok(())
    }
}

