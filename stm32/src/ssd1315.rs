use embassy_stm32::i2c::{Error};
use embedded_graphics::{draw_target::DrawTarget, pixelcolor::BinaryColor};
use embedded_graphics::prelude::*;
use embedded_hal_async::i2c::Operation;
use crate::{I2C};
use crate::animation::FlushableDisplay;

pub struct SSD1315 {
    framebuffer: [u8; 128 * 8],
}

enum SetAddress {
    Page{start : u8, end : u8},
    Column{start : u8, end: u8},
}

enum WriteType<'a> {
    Command(&'a[u8]),
    Data(&'a[u8]),
}

impl SSD1315 {
    const ADDRESS : u8 = 0x3C;
    async fn write_raw<'a>(t : WriteType<'a>) -> Result<(), Error> {
        let [ctrl,data] = match t{
            WriteType::Command(data) => [&[0x00], data],
            WriteType::Data(data) => [&[0x40], data],
        };
        I2C.lock().await.as_mut().unwrap().transaction(Self::ADDRESS, &mut [
            Operation::Write(ctrl),
            Operation::Write(data),
        ]).await
    }

    async fn set_addr(&mut self, address : SetAddress) -> Result<(), Error>{
        let cmd = match address {
            SetAddress::Page { start, end } => [
                0x22,
                start & 0x07,
                end & 0x07,
            ],
            SetAddress::Column { start, end } => [
                0x21,
                start & 0x7F,
                end & 0x7F,
            ]
        };
        Self::write_raw(WriteType::Command(&cmd)).await
    }

    pub fn set_pixel(&mut self, x :u8, y :u8, value : bool) {
        assert!(x < 128 && y < 64);
        let target : usize = (usize::from(y)/8) * 128 + usize::from(x);
        let target = unsafe {self.framebuffer.get_unchecked_mut(target)};
        if value {
            *target |= 0x1 << (y % 8);
        } else {
            *target &= !(0x1 << (y % 8));
        };
    }

    pub async fn sleep(&self) -> Result<(), Error> {
        Self::write_raw(WriteType::Command(&[0xAE])).await
    }

    pub async fn wake(&self) -> Result<(), Error> {
        Self::write_raw(WriteType::Command(&[0xAF])).await
    }

    pub async fn init() -> Result<Self, Error> {
        let ssd1315 = Self {
            framebuffer : [0; 128 * 8],
        };

        let cmds : [u8; _] = [
            0xA8, 0x3F, // Set Mux Ratio
            0xD3, 0x00, // Set Display offset
            0x20, 0x00, // Set Adressing mode to vertical
            0x40,       // Set start line
            0xA1,       // Set segment re-map / 0xA0
            0xC8,       // Set COM output scan direction / 0xC0
            0xDA, 0x12, // Set COM pin hardware configuration
            0x81, 0x7F, // Set contrast
            0xA4,       // Resume the display
            0xD5, 0x80, // Set Oscillator frequency
            0x8D, 0x14, // Enable Charge pump
            0xAF        // Turn the display on
        ];

        Self::write_raw(WriteType::Command(&cmds)).await?;
        Ok(ssd1315)
    }
}

impl DrawTarget for SSD1315 {
    type Color = BinaryColor;
    type Error = Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>
    {
        for Pixel(coord, color) in pixels.into_iter() {
            if let Ok((x @ 0..128, y @ 0..64)) = coord.try_into() {
                self.set_pixel(x as u8, y as u8, color.is_on());
            }
        };
        Ok(())
    }
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.framebuffer.fill(match color.is_on() {
            true => 0xff,
            false => 0
        });
        Ok(())
    }
}

impl OriginDimensions for SSD1315 {
    fn size(&self) -> Size {
        Size {
            width : 128,
            height: 64
        }
    }
}

impl FlushableDisplay for SSD1315 {
    async fn flush(&mut self) -> Result<(), Error> {
        self.set_addr(SetAddress::Column { start: 0, end: 127 }).await.expect("Flush: set column shouldn't err");
        self.set_addr(SetAddress::Page { start: 0, end: 7 }).await.expect("Flush: set page shouldn't err");

        let framebuffer = &self.framebuffer;
        // let mut f_debug: heapless::Vec<u8, 8> = heapless::Vec::new();
        // for f in framebuffer {
        //     f_debug.push(f.clone()).expect("should always success");
        //     if f_debug.is_full()  {
        //         info!("{:?},", f_debug);
        //         f_debug.clear();
        //     }
        // }
        Self::write_raw(WriteType::Data(framebuffer)).await
    }
}
