use core::{array, cell::RefCell, iter, marker::PhantomData};

use defmt::info;
use embassy_stm32::{exti::ExtiInput, gpio::Output, mode::Async, spi::{Spi, mode::Master}};
use embassy_time::{Duration, TimeoutError, Timer, WithTimeout};
use heapless::{String, Vec};

pub mod task;

const READ_BUFFER_LEN : usize = 2048;

#[derive(Debug)]
pub struct InvalidResponseLength {
    length : usize,
    expected : usize,
}

#[derive(Debug)]
pub enum RawError {
    TimeoutError,
    InvalidResponseLength(InvalidResponseLength),
}
impl From<TimeoutError> for RawError {
    fn from(_: TimeoutError) -> Self {
        Self::TimeoutError
    }
}

pub enum Error {
    TimeoutError,
    SpiSuccessButESPFail, // esp fail to execute command
    InvalidResponseLength(InvalidResponseLength),
    UnexpectedResponse(u8),
}
impl From<RawError> for Error {
    fn from(value: RawError) -> Self {
        match value {
            RawError::TimeoutError => Self::TimeoutError,
            RawError::InvalidResponseLength(r) => Self::InvalidResponseLength(r),
        }
    }

}

enum Commands {
    Scan = 0x0,
    Connect = 0x1,
    SendClimate = 0x2,
    Sleep = 0x3
}

#[derive(Clone)]
pub struct ConnWifi {
    pub ssid : String<32>,
    pub password : String<63>,
}

#[derive(Debug, defmt::Format)]
pub struct WifiScan {
    ssid : String<32>,
    rssi : u8,
}

pub struct ESP12F {
    state : Option<ESP12FState>,
}

impl ESP12F {
    pub fn new(spi: Spi<'static, Async, Master>, spi_handshake :ExtiInput<'static, Async>, spi_cs: Output<'static>, reset_pin : Output<'static> )->Self{
        let state = Some(
            ESP12FState::Off(ESP12FOff{
                spi,
                reset_pin,
                spi_handshake,
                spi_cs
            })
        );
        Self{ state}
    }
    pub async fn borrow_active_mut<'a>(&'a mut self)->&'a mut ESP12FActive{
        let esp = self.state.take().expect("state should always exist");
        match esp {
            ESP12FState::On(esp) => {
                self.state = Some(ESP12FState::On(esp));
                if let Some(ESP12FState::On(s)) = &mut self.state {
                    s
                } else {panic!("this should never reached")}
            },
            ESP12FState::Off(esp) => {
                let active = esp.turn_on().await;
                self.state = Some(ESP12FState::On(active));
                if let Some(ESP12FState::On(s)) = &mut self.state {
                    s
                } else {panic!("this should never reached")}
            }
        }
    }
    pub async fn turn_off(&mut self) {
        let esp = self.state.take().expect("state should always exist");
        self.state = match esp {
            ESP12FState::On(esp) => {
                Some(ESP12FState::Off(esp.sleep().await))
            },
            ESP12FState::Off(esp) => {
                Some(ESP12FState::Off(esp))
            }
        }
    }
}

pub enum ESP12FState {
    Off(ESP12FOff),
    On(ESP12FActive)
}


pub struct ESP12FOff {
    spi : Spi<'static, Async, Master>,
    spi_handshake : ExtiInput<'static, Async>,
    spi_cs : Output<'static>,
    reset_pin : Output<'static>,
}

impl<'a> ESP12FOff {
    pub async fn turn_on(mut self)->ESP12FActive{
        self.spi_cs.set_low();
        self.reset_pin.set_low();
        Timer::after_millis(50).await;
        self.reset_pin.set_high();
        Timer::after_millis(50).await;
        self.spi_cs.set_high();
        Timer::after_millis(500).await;
        let mut esp = ESP12FActive{
            spi_cs : self.spi_cs,
            spi_handshake : self.spi_handshake,
            spi : self.spi,
            reset_pin : self.reset_pin
        };
        esp.write_status(0x0).await; // stupid hack, for some idiotic reason beyond comprehension esp always append
                                     // 0x0080 at first write to status register, so this needed to clear that forsaken behaviour
        esp
    }
}

type ScanReturn = Result<Vec<WifiScan, 10>, Error>;
type ConnectReturn = Result<(), Error>;
type SendClimateReturn = Result<(), Error>;

pub struct ESP12FActive {
    spi : Spi<'static, Async, Master>,
    spi_handshake : ExtiInput<'static, Async>,
    spi_cs : Output<'static>,
    reset_pin : Output<'static>,
}

impl ESP12FActive {
    async fn write_status(&mut self, length : u32) {
        let write_bit: [u8;_] = [0x1];
        let mut data = write_bit.into_iter().chain(length.to_le_bytes().into_iter());
        let data: [u8; 5] = array::from_fn(|_| data.next().unwrap());
        self.spi_cs.set_low();
        self.spi.write(&data).await.unwrap();
        self.spi_cs.set_high();
        if length == 0 {
            return;
        }
        self.spi_handshake.wait_for_high().await;
    }
    async fn read_status(&mut self, timeout : Duration) -> Result<u32,TimeoutError> {
        self.spi_handshake.wait_for_high()
            .with_timeout(timeout).await?;
        let read_bit: [u8;_] = [0x4, 0x0, 0x0, 0x0, 0x0];
        let mut read_data = [0; 5];

        self.spi_cs.set_low();
        self.spi.transfer(&mut read_data, &read_bit).await.unwrap();
        self.spi_cs.set_high();
        Ok(u32::from_le_bytes([read_data[1], read_data[2], read_data[3], read_data[4]]))
    }
    async fn read<const LEN: usize>(&mut self, timeout : Duration) -> Result<Vec<u8, LEN>, RawError> {
        let data_len = loop {
            let data_len = self.read_status(timeout).await?;
            match data_len {
                0xffff_ffff | 0x0 => Timer::after_millis(20).await,
                _ => break data_len as usize,
            } // hacky way to fix if read too early after esp boot, it will return 0 or max u32
              // TODO: make esp handshake ceremony at esp startup instead
        };
        if data_len <= LEN {
            return Err(RawError::InvalidResponseLength(InvalidResponseLength { length: data_len, expected: LEN }))
        }
        assert!(data_len <= LEN);

        let mut parsed : Vec<u8, LEN> = Vec::new();

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
    async fn write(&mut self, data: &[u8]) -> Result<(), RawError> {
        self.write_status(data.len() as u32).await;
        let mut packet = [0u8; 66];
        packet[0] = 0x02;
        packet[1] = 0x00;
        for d in data.chunks(64) {
            self.spi_handshake.wait_for_high().await;

            packet[2..2 + d.len()].copy_from_slice(d);

            self.spi_cs.set_low();
            let data = &packet[..2 + d.len()];
            self.spi.write(data).await.unwrap();
            self.spi_cs.set_high();
        }
        self.spi_handshake.wait_for_high().await;
        self.write_status(0).await;

        Ok(())
    }
    pub async fn scan(&mut self)-> ScanReturn{
        self.write(&[Commands::Scan as u8]).await?;
        let parsed  = self.read::<2048>(Duration::from_millis(10)).await?;
        let mut result: Vec<WifiScan, 10> = Vec::new();
        for chunk in parsed.chunks(34) {
            let ssid_bytes = &chunk[..32];
            let ssid_bytes = ssid_bytes
                .into_iter()
                .take_while(|x| **x != '\0' as u8)
                .copied();

            let ssid : Vec<u8,32> = Vec::from_iter(ssid_bytes);
            let ssid: String<32> = match String::from_utf8(ssid) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let rssi = chunk[33];
            result.push(WifiScan { ssid, rssi }).unwrap();
        };
        Ok(result)
    }
    pub async fn connect(&mut self, wifi: ConnWifi)->ConnectReturn{

        let mut data = [Commands::Connect as u8].into_iter()
            .chain(wifi.ssid
                .as_bytes()
                .into_iter()
                .copied()
                .chain(iter::repeat(0))
                .take(33))
            .chain(wifi.password
                .as_bytes()
                .into_iter()
                .copied()
                .chain(iter::repeat(0))
                .take(64));
        let data : [u8; 98] = array::from_fn(|_| data.next().unwrap());
        self.write(&data).await?;
        let status = self.read::<1024>(Duration::from_secs(15)).await?;
        match status[0] {
            0x0 => Ok(()),
            0x3 => Err(Error::SpiSuccessButESPFail),
            any => Err(Error::UnexpectedResponse(any)),
        }
    }
    async fn sleep(mut self) -> ESP12FOff {
        self.write(&[Commands::Sleep as u8]).await.unwrap();
        Timer::after_millis(50).await;
        ESP12FOff{
            spi : self.spi,
            spi_handshake : self.spi_handshake,
            spi_cs : self.spi_cs,
            reset_pin : self.reset_pin,
        }
    }
}

